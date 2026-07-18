# Runtime fingerprint surface

Technical reference: what anti-bot scripts actually probe inside the JavaScript
runtime. These are not abstract spec questions — each one is a high-signal check
that runs in production detection code.

---

## 1. Error stack traces

### V8 format (Chrome, Edge, Node.js)

```
Error: message
    at functionName (file.js:line:col)
    at Object.method [as alias] (file.js:line:col)
    at new Constructor (file.js:line:col)
    at file.js:line:col                    (global scope, no function)
    at async asyncFn (file.js:line:col)   (async frames, V8 ≥ 7.3)
```

The first line is `ErrorType: message`. Every frame line starts with exactly
four spaces then `at `. Column numbers are 1-based. The format is:

- `at Type.functionName [as methodName]` when type, function name, and method
  name are all available.
- `at functionName (location)` when there's no `Type` (global scope).
- `at new Constructor (location)` for construct calls.
- `at async functionName (location)` for async stack frames.
- `at eval (eval at <parent> (parent.js:1:1), <anonymous>:1:8)` for eval.
- Location `native` for internal V8 frames.

### Format comparison

| Engine              | Frame prefix    | Column info | Error header | Exposes `stackTraceLimit` |
|---------------------|-----------------|-------------|--------------|---------------------------|
| V8 (Chrome/Node)   | `    at fn (f:l:c)` | Yes (1-based) | `Error: msg` | Yes (number) |
| SpiderMonkey (FF)  | `fn@file:l:c`   | Yes | No header, just `msg` | No |
| JavaScriptCore (Safari) | `fn@file:l:c` | Yes | `msg` | No |

The TC39 Error Stacks proposal (Stage 1) standardizes that `stack` exists but
explicitly leaves the textual format *implementation-defined*. The divergence
is a designed-in degree of freedom — it will not be smoothed away.

### Detection usage

A detector throws a controlled error and parses `error.stack`. If the UA says
Safari but the stack reads like V8 (`    at ` frames), the UA is spoofed and
the engine is V8. This costs essentially nothing and fires on every visit.

### `Error.captureStackTrace(error, constructorOpt)`

- V8-only. Installs a `stack` property on any object (not just Error instances).
- In V8, it materializes as a **getter/setter pair** on the object.
- In SpiderMonkey and JavaScriptCore, it's a **writable data property**.
- The second argument trims frames above and including `constructorOpt`.

### `Error.stackTraceLimit`

- **V8 only.** A numeric data property on `Error` itself, default `10`.
- Setting it to `0` disables stack collection; `Infinity` collects all frames.
- Absent in SpiderMonkey. `Error.stackTraceLimit` returns `undefined`.
- JavaScriptCore added compatibility support, so it may exist there too.
- Detection check: `typeof Error.stackTraceLimit === 'number'` → V8.

### `Error.prepareStackTrace(error, structuredTrace)`

- V8-only hook. If assigned, replaces the default stack stringifier.
- The `structuredTrace` is an array of CallSite objects.
- Detection scripts can detect if someone overrides this to hide their own
  frames.

### Property descriptor of `error.stack` (V8)

- Lives on the **instance** as a data property (configurable: true, writable:
  true, enumerable: false).
- Materialized lazily on first access, then cached.
- In SpiderMonkey, `stack` is an **accessor on `Error.prototype`**, not on the
  instance. This is directly observable via `Object.getOwnPropertyDescriptor`.

---

## 2. `Function.prototype.toString()`

### Native function format (Chrome)

```
function name() { [native code] }
```

Key details:

- The **exact** string is `function name() { [native code] }` with exactly one
  space before `{`, one space after `{`, one space before `}`, and a space
  between `function` and the name.
- For getters: `function get name() { [native code] }`.
- For setters: `function set name() { [native code] }`.
- The `[native code]` token is fixed — no engine-level whitespace variation.
- The `()` part is always empty for native functions (no parameter names shown).
- V8 version 12+ normalized to `function () { [native code] }` in some paths;
  test against your target.

### `navigator.webdriver.toString()`

In real, non-automated Chrome:

```
undefined
```

`navigator.webdriver` is a getter that returns `false` or `undefined`, but it
is **not** itself a function. Calling `.toString()` on the value yields the
string representation of the boolean/undefined, **not** `[native code]`.

Detection scripts actually check:

```js
const desc = Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver');
// desc.get.toString() → "function get webdriver() { [native code] }"
```

If the getter was replaced with a JS function:

```js
// Patched version:
Object.defineProperty(Navigator.prototype, 'webdriver', {
  get: () => false
});
desc.get.toString()  // "() => false" ← instant detection
```

### Methods that detection scripts toString() check

Kasada's `ips.js` (paraphrased) checks ~20 methods; these are the most
common:

| Method | Native toString |
|--------|----------------|
| `navigator.webdriver` getter (on Navigator.prototype) | `function get webdriver() { [native code] }` |
| `HTMLCanvasElement.prototype.toDataURL` | `function toDataURL() { [native code] }` |
| `HTMLCanvasElement.prototype.getContext` | `function getContext() { [native code] }` |
| `WebGLRenderingContext.prototype.getParameter` | `function getParameter() { [native code] }` |
| `CanvasRenderingContext2D.prototype.getImageData` | `function getImageData() { [native code] }` |
| `Permissions.prototype.query` | `function query() { [native code] }` |
| `Notification.requestPermission` | `function requestPermission() { [native code] }` |
| `Function.prototype.toString` | `function toString() { [native code] }` |

### The meta-check: patching toString itself

If a stealth plugin patches `Function.prototype.toString` to lie about other
functions, the detector escalates:

```js
Function.prototype.toString.toString()
// Native: "function toString() { [native code] }"
// Patched: returns the patcher's own source
```

A detector also fetches a pristine `toString` from a fresh `<iframe>`:

```js
const iframe = document.createElement('iframe');
document.body.appendChild(iframe);
const cleanToString = iframe.contentWindow.Function.prototype.toString;
cleanToString.call(suspectFn);
// Returns real source, bypasses parent-window patches
```

---

## 3. `toString()` of specific APIs

### `HTMLCanvasElement.prototype.getContext.toString()`

```
function getContext() { [native code] }
```

Headless Chrome / SwiftShader: the `toString()` itself is native, but
`getContext('webgl')` returns a `WebGLRenderingContext` whose `getParameter`
may reveal software rendering (ANGLE, SwiftShader). The `toString()` of the
function is correct; the rendering output leaks.

### `WebGLRenderingContext.prototype.getParameter.toString()`

```
function getParameter() { [native code] }
```

Same pattern — the function is native, but calling it with
`GL_VENDOR`/`GL_RENDERER` reveals the real GPU path. Real Chrome on Windows
returns ANGLE + D3D11; headless returns SwiftShader or native GPU.

### `navigator.mediaDevices.enumerateDevices.toString()`

```
function enumerateDevices() { [native code] }
```

Headless/virtualized environments may have no media devices, causing the
promise to resolve to an empty array. A detection script checks both the
function's `toString()` *(must be native)* and the result *(must have at
least one audio device on real hardware)*.

### Headless vs real Chrome

The `toString()` of these methods is **always native** in genuine Chrome,
whether headless or headed. The difference between headless and headed is
not in the `toString()` signature, but in:

- Whether the getter was replaced with a JS function (stealth plugins)
- Whether a `Proxy` wrapper was applied
- The **return values** of the function (empty lists, software renderer
  strings, missing codec support)

If `Function.prototype.toString.call(fn).includes('[native code]')` is
`false`, the function **was patched in JavaScript** — regardless of whether
the browser is headless. This catches `puppeteer-extra-plugin-stealth`,
`undetected-chromedriver`, and similar JS-layer patches.

---

## 4. Object property descriptor differences

### `navigator.webdriver`

**Real Chrome (non-automated):**

```js
Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')
// → { get: ƒ, set: undefined, enumerable: true, configurable: true }
//   get.toString() → "function get webdriver() { [native code] }"

Object.getOwnPropertyDescriptor(navigator, 'webdriver')
// → undefined   (it's on the prototype, not the instance)
```

**Automated Chrome (unpatched):**

```js
Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')
// → { get: ƒ, set: undefined, enumerable: true, configurable: true }
//   get.toString() → "function get webdriver() { [native code] }"
//   get.apply(navigator) → true
```

**Naively patched (stealth plugin via `defineProperty`):**

```js
Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')
// → { get: () => undefined, set: undefined, enumerable: true, configurable: true }
//   get.toString() → "() => undefined"   ← DIFFERENT
```

Detection script class pattern:

```js
const d = Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver');
if (d !== undefined) {
  // Real Chrome: this path doesn't execute — no own descriptor on prototype
  // Automated Chrome: descriptor exists
  const isNative = d.get.toString().includes('[native code]');
  const value = d.get.apply(navigator);
}
```

**Important subtlety**: In some Chrome versions, the `webdriver` property may
not exist as an own property of `Navigator.prototype` at all in non-automated
mode. The detection is: **property exists + getter is native + value is
`true`** → automated. **Property exists + getter is JS** → JS-layer patch
detected. **No property at all** → likely non-automated.

### `navigator.languages`

```js
Object.getOwnPropertyDescriptor(Navigator.prototype, 'languages')
// → { get: ƒ, set: undefined, enumerable: true, configurable: true }
//   get.toString() → "function get languages() { [native code] }"

// On the navigator instance:
Object.getOwnPropertyDescriptor(navigator, 'languages')
// → undefined (inherited from prototype)

navigator.languages
// → ["en-US", "en", "zh-CN"]  (ordered by preference)
```

**Key tells:**
- The getter must be native (`[native code]`).
- The property must be on `Navigator.prototype`, not on `navigator` itself.
- The descriptor shape: `{ get, set: undefined, enumerable: true, configurable: true }`.
- The `languages` array must contain at least one entry and be frozen
  (`Object.isFrozen(navigator.languages)` is `true` in real Chrome).
- The first element must match `navigator.language`.
- The values must be BCP 47 tags.

### Common Navigator.prototype descriptor pattern

All standard Navigator properties follow the same shape:

```js
{
  get: ƒ nativeGetter(),
  set: undefined,
  enumerable: true,
  configurable: true
}
```

Accessor properties (getter-only) include: `webdriver`, `languages`,
`language`, `platform`, `userAgent`, `vendor`, `plugins`, `mimeTypes`,
`hardwareConcurrency`, `deviceMemory`, `onLine`, `product`, `appVersion`,
`appName`, `cookieEnabled`, `mediaDevices`, `permissions`, `geolocation`,
`credentials`, `clipboard`, `connection`, `serviceWorker`, `storage`,
`presentation`, `mediaCapabilities`, `keyboard`, `locks`, `mediaSession`,
`maxTouchPoints`.

If any of these has a `value` descriptor on the instance (rather than a
`get` on the prototype), it was overwritten.

---

## 5. `constructor` and `prototype` detection

### Prototype chain probing

Detection scripts walk prototype chains to find patched surfaces that
JavaScript-layer spoofs cannot fully hide.

**PluginArray check:**

```js
navigator.plugins instanceof PluginArray
// Real Chrome: true
// Fabricated (Object/Proxy): false
```

The fabricated object's prototype is `Object.prototype` or a Proxy, not the
native `PluginArray` C++ binding. `instanceof` checks the prototype chain
against `PluginArray.prototype`, which the fake lacks.

**Function constructor check:**

```js
suspectFn.constructor === Function
// Native: true  (all functions have Function as constructor)
// Proxy:  false (Proxy constructor !== Function)
```

**Prototype of getter:**

```js
const d = Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver');
Object.getPrototypeOf(d.get) === Function.prototype;
// Native: true
// JS override: true (both are Function)
// But d.get.toString() reveals the difference
```

**Cross-realm identity check:**

```js
const iframe = document.createElement('iframe');
document.body.appendChild(iframe);

navigator.plugins instanceof iframe.contentWindow.PluginArray
// Real Chrome: false (different realms, different prototypes)
// JS fabrication: false (doesn't have PluginArray at all)

// But more useful:
iframe.contentWindow.navigator.webdriver
// Main world patched → undefined
// Iframe (not patched by init script) → true
```

This is the most reliable detector of JS-layer patches: create a fresh
iframe, read `contentWindow.navigator.webdriver` before the stealth script's
iframe patcher runs. If the iframe reports `true` while the main window
reports `undefined`, the browser's native getter is intact — only the main
world was patched.

**`hasOwnProperty` check for spoofed methods:**

```js
// Native functions don't have 'toString' as an own property
Function.prototype.toString.hasOwnProperty('toString')
// → false

// But if you wrapped native toString in a Proxy, it's now an own property
// of the proxy, making this return true when it shouldn't.
```

**`failsTypeError` check:**

Native getters on `Navigator.prototype` throw a specific `TypeError` when
called with the wrong `this`. A naive JS shim may silently return a value,
and that silence is the signal:

```js
try {
  const getter = Object.getOwnPropertyDescriptor(Navigator.prototype, 'vendor').get;
  getter.call({});  // wrong `this`
  // If we get here without error → the getter is a JS shim, not native
} catch (e) {
  // Native getters throw TypeError → expected
}
```

### `constructor` of `Error`

```js
new Error().constructor === Error   // true
Error.prototype.constructor === Error // true
```

Detection scripts check that `error.constructor.name` matches the expected
engine. A patched error's constructor could differ.

---

## References

- V8 Stack Trace API: https://v8.dev/docs/stack-trace-api
- MDN `Error.prototype.stack`: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error/stack
- Castle.io (2025): Why a classic CDP bot detection signal suddenly stopped working
- sveba (2026): How V8 Leaks Your Headless Browser's Identity — the `.stack`
  getter and prototype-Proxy `ownKeys` console side channels
- CreepJS: https://creepjs.org/ (live fingerprint tester)
- BotScanner / Sannysoft: https://bot.sannysoft.com/ (live headless detection)
