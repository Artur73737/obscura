# Headless Chrome: Implementation & Fingerprinting Technical Reference

> Status: Compiled July 2026 from Chromium source, security research, and anti-bot industry analysis.

---

## 1. Chrome Headless Mode Implementation Details

### `--headless=new` (Chrome 112+) vs `--headless=old`

| Aspect | `--headless=old` | `--headless=new` |
|---|---|---|
| **Architecture** | Separate application layer in `//headless/` — a lightweight wrapper around `//content` module | Full Chrome browser (`//chrome/`) running without visible UI — shares all browser code |
| **Dependencies** | Minimal — no X11/Wayland, no D-Bus required | Full — requires X11/Wayland on Linux, D-Bus, the same dependency set as headed Chrome |
| **Performance** | Faster, lower memory/CPU — no UI compositor overhead | Slower, more resource-heavy — same overhead as headed Chrome minus the display |
| **Extensions** | Not supported | Fully supported |
| **`chrome://gpu`** | Separate, simplified GPU pipeline | Same as headed Chrome |
| **Availability** | Removed from Chrome binary as of Chrome 132 (Jan 2025). Now distributed as standalone `chrome-headless-shell` binary | Default since Chrome 128+; `--headless` now maps to `--headless=new` |
| **Security model** | Separate process model, fewer attack surfaces | Full Chrome security model (sandbox, GPU process, etc.) |

**Chromium's own description**: *"The new headless mode is Chrome browser running without any visible UI."*

Since Chrome 132 (released Jan 2025), `--headless=old` prints an error message. Use `chrome-headless-shell` for the old implementation.

### Renderer Backend

**New headless (`--headless=new`):**
- **Default:** Uses SwANGLE (ANGLE + SwiftShader Vulkan) as the OpenGL ES driver
- On Linux with X11, can use GPU hardware via `--enable-gpu` (requires X display)
- On Linux without X11, can use Vulkan backend: `--use-angle=vulkan`
- As of Chrome 128+, SwiftShader is **not** the automatic WebGL fallback by default
- **WebGL now requires explicit opt-in:** `--enable-unsafe-swiftshader` (or real GPU passthrough)

**Old headless (`chrome-headless-shell` / `--headless=old`):**
- Used legacy SwiftShader GL (being phased out) or SwANGLE
- Lighter GPU stack with fewer 3D capabilities by default

### Disabled GPU Features in Headless Mode

By default, headless mode (both old and new) forces software rendering. The following are disabled unless explicitly overridden:

| Feature | Default State |
|---|---|
| GPU compositing | Disabled (software compositing fallback) |
| WebGL | Disabled unless `--enable-unsafe-swiftshader` or GPU passthrough |
| WebGL2 | Same as WebGL |
| Video decode | Unavailable |
| Vulkan | Disabled |
| Hardware rasterization | Disabled |
| OOP rasterization | Disabled |

Flags to re-enable: `--enable-gpu` (forces GPU on Linux with X11), `--ignore-gpu-blocklist`, `--enable-unsafe-swiftshader` (for WebGL in new headless).

### Font Availability

**Default headless (Docker/server):**
- Minimal font set: typically only `DejaVu` family (DejaVu Sans, DejaVu Serif, DejaVu Sans Mono)
- No CJK fonts unless explicitly installed
- No emoji fonts (e.g., Noto Color Emoji) unless installed
- `fc-list` on a minimal container shows ~10-20 fonts vs. hundreds on a headed desktop

**Headed Chrome (desktop):**
- Full OS font set: hundreds of fonts
- CJK fonts (Noto, MS Gothic, etc.)
- Emoji fonts
- Application-specific fonts (Office, Adobe, etc.)

**Fingerprinting implication:** `document.fonts` API and font enumeration via canvas text metrics expose a dramatically smaller font pool in headless/server environments. This is a high-entropy signal used by services like Cloudflare Turnstile and DataDome.

---

## 2. JavaScript Properties That Differ Between Headless and Headed Chrome

### Default values (unmodified, no stealth patches)

| Property | Headless (default) | Headed Chrome (desktop) |
|---|---|---|
| `navigator.userAgent` | Contains `HeadlessChrome/X` | Contains `Chrome/X` |
| `navigator.webdriver` | `true` | `undefined` |
| `navigator.plugins.length` | `0` (empty PluginArray) | `5` (Chrome PDF Plugin, Chrome PDF Viewer, Native Client, etc.) |
| `navigator.mimeTypes.length` | `0` | `4+` |
| `navigator.languages` | `[]` (empty array) or `['en-US', 'en']` | User-configured languages |
| `navigator.hardwareConcurrency` | Varies (often 1-4 on server VPS) | 4-16+ (matches real CPU) |
| `navigator.deviceMemory` | Often `0` or `2` (server) | `4`-`8`+ typical |
| `window.chrome` | Sparse or missing `runtime`; `loadTimes` may be undefined | Full `chrome` object with `runtime`, `loadTimes()`, `csi()`, `app` |
| `window.chrome.runtime` | `undefined` | Object with `connect`, `sendMessage`, `getManifest`, etc. |
| `Notification.permission` | Often `'denied'` | `'prompt'` or `'granted'` |
| `navigator.permissions.query({name:'notifications'})` | Inconsistency with `Notification.permission` | Consistent |
| Speech synthesis | Fewer or zero Google voices | Many Google voices |

### Canvas Rendering Differences

- Canvas `toDataURL()` fingerprint hash differs significantly between GPU-backed and SwiftShader rendering
- Sub-pixel rendering, anti-aliasing, and font metrics differ between SwiftShader and native GPU drivers
- The same canvas draw operations produce different pixel output on SwiftShader vs. real GPU

### WebGL Vendor/Renderer Strings

| Environment | `UNMASKED_VENDOR_WEBGL` | `UNMASKED_RENDERER_WEBGL` |
|---|---|---|
| Headless (default, SwiftShader) | `"Google Inc. (Google)"` | `"Google SwiftShader"` or `"ANGLE (Google, Vulkan 1.3.0 (SwiftShader Shader Model 5.0))"` |
| Headless (SwANGLE) | `"Google Inc. (Google)"` | `"ANGLE (Google, Vulkan 1.3.0 (SwiftShader Shader Model 5.0))"` |
| Headless (Mesa llvmpipe) | `"Mesa"` | `"Mesa OffScreen"` or `"llvmpipe (LLVM 15.0.7, 128 bits)"` |
| Headed Windows (NVIDIA) | `"Google Inc. (NVIDIA Corporation)"` | `"ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0)"` |
| Headed macOS (Apple M2) | `"Google Inc. (Apple)"` | `"ANGLE (Apple, Apple M2, OpenGL 4.1)"` |
| Headed Linux (Intel) | `"Google Inc. (Intel)"` | `"ANGLE (Intel, Intel Iris OpenGL Engine, OpenGL 4.6)"` |

### MediaDeviceInfo Differences

- `navigator.mediaDevices.enumerateDevices()` returns **no audio/video input devices** in headless (no cameras, no microphones)
- Headed Chrome returns real device labels (e.g., "USB Camera", "Built-in Microphone")
- Audio context fingerprinting (via `OfflineAudioContext`) produces different results due to missing audio hardware and different sample rate handling

### Screen Size Differences

| Property | Headless (default) | Headed Chrome |
|---|---|---|
| `screen.width` x `screen.height` | `800` x `600` | Real monitor resolution (e.g., 1920x1080) |
| `screen.availWidth` x `screen.availHeight` | `800` x `600` (same — no taskbar) | Real available area |
| `window.outerWidth` x `window.outerHeight` | `800` x `600` | Includes browser chrome |
| `window.innerWidth` x `window.innerHeight` | `800` x `600` (identical to outer — no chrome) | Less than outer (browser UI subtracted) |
| `window.devicePixelRatio` | `1` | Usually `1`, `1.25`, `1.5`, `2`, or `3` |
| `screen.availLeft` / `availTop` | `0` / `0` | May reflect multi-monitor config |

**Key tell:** In headless, `window.outerWidth === window.innerWidth` and `screen.width === screen.availWidth`. On a real desktop, browser chrome and OS taskbars create differences between these values. This is a heavily checked signal.

---

## 3. GPU/Rendering Info: `--headless=new` vs `--headless=old`

| Signal | `--headless=old` | `--headless=new` |
|---|---|---|
| GPU process | Separate, stripped-down GPU process | Full Chrome GPU process (same as headed) |
| WebGL by default | Yes (via legacy SwiftShader GL) | **No** — requires `--enable-unsafe-swiftshader` |
| WebGL renderer | `Google SwiftShader` (Direct GL) | `ANGLE (Google, Vulkan 1.3.0 (SwiftShader Shader Model 5.0))` |
| `chrome://gpu` output | Simplified, fewer entries | Full GPU info page, same as headed |
| Skia renderer | Not used by default | `UseSkiaRenderer` can be enabled |
| Canvas acceleration | Software only | Can be hardware-accelerated with GPU passthrough |
| Max texture size | Limited (SwiftShader-dependent) | Same as headed with GPU |
| Shader precision | Lower (SwiftShader defaults) | Hardware-dependent |

**New headless is more realistic** because `chrome://gpu` and WebGL info pages look identical to headed Chrome — except the renderer string still says SwiftShader unless a real GPU is provided.

---

## 4. `--disable-gpu` and Its Effect on Fingerprinting

`--disable-gpu` forces software rendering even in headed mode. Impact:

- **Canvas fingerprinting:** `toDataURL()` output changes (CPU rasterization differs from GPU)
- **WebGL:** Completely disabled (context creation fails)
- **WebGL2:** Completely disabled
- `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL`: Returns `null` or empty
- `canvas.getContext('webgl')` returns `null`
- **CSS 3D transforms:** Fall back to software
- **`rgba()` handling:** May differ in precision
- **Performance.now()** timestamps: GPU-related timing noise removed (measureable via timing analysis)

**Detection value:** A browser claiming "Chrome 126 on Windows 11" with WebGL returning null is extremely suspicious. Real headed Chrome on any modern OS supports WebGL.

---

## 5. `--enable-features=Vulkan` in Headless Chrome

- Forces Chromium to use Vulkan for GPU rendering (instead of OpenGL/ANGLE)
- In headless without a real GPU, Vulkan falls back to SwiftShader Vulkan (software)
- On Linux with NVIDIA GPU: `--enable-features=Vulkan --use-angle=vulkan` can enable **real hardware GPU** in headless mode (requires X11 or `--use-vulkan=native`)
- On Windows: Vulkan support in headless is limited
- The WebGL renderer string changes from `ANGLE (Google, SwiftShader, ...)` to a Vulkan-based SwiftShader string
- **Without GPU:** WebGL renderer becomes `Google SwiftShader` via Vulkan, still detectable
- **With NVIDIA GPU passthrough:** WebGL renderer becomes the real GPU (e.g., `NVIDIA GeForce RTX 4090`), dramatically improving fingerprint realism

**Critical note (2026):** SwiftShader WebGL fallback is deprecated and requires `--enable-unsafe-swiftshader` opt-in since Chrome 128+. Without this flag or real GPU passthrough, WebGL simply fails in headless.

---

## 6. Google SwiftShader: Deep Dive

### What It Is

SwiftShader is an open-source, high-performance implementation of the Vulkan 1.3 and OpenGL ES 3.1+ graphics APIs that **runs entirely on the CPU**. It uses JIT compilation to execute shader code on CPU cores, mimicking a GPU in software.

### How It Renders in Headless

1. **New headless (default path):** SwANGLE = ANGLE (translates OpenGL ES to Vulkan) + SwiftShader Vulkan (executes Vulkan on CPU). Two-layer abstraction: WebGL → OpenGL ES → ANGLE → Vulkan → SwiftShader → CPU
2. **Old headless:** Legacy SwiftShader GL — direct OpenGL ES implementation on CPU
3. Both produce visually correct output but with different performance characteristics and pixel-level floating-point behavior

### WebGL Strings SwiftShader Produces

| API | String |
|---|---|
| `UNMASKED_VENDOR_WEBGL` | `Google Inc. (Google)` |
| `UNMASKED_RENDERER_WEBGL` (SwANGLE path) | `ANGLE (Google, Vulkan 1.3.0 (SwiftShader Shader Model 5.0))` |
| `UNMASKED_RENDERER_WEBGL` (legacy GL path) | `Google SwiftShader` |
| `VERSION_WEBGL` | `WebGL 1.0 (OpenGL ES 3.0 SwiftShader)` or `WebGL 1.0 (OpenGL ES 3.0 ANGLE + SwiftShader)` |
| `RENDERER_WEBGL` | `ANGLE (Google, SwiftShader Shader Model 5.0)` |

### Key Fingerprinting Properties of SwiftShader

- **Draw timing:** CPU-based rendering is 10-100x slower than hardware GPU — measureable via `performance.now()`
- **Precision:** Lower floating-point precision in shader math — produces different WebGL canvas fingerprints than any real GPU
- **Max texture size:** Typically 16384 or 8192 (vs. 16384+ for real GPUs — similar but timing differs)
- **Extensions supported:** Subset of what real GPUs expose; some extensions (e.g., `WEBGL_compressed_texture_s3tc`) may be missing
- **No hardware video decoding:** `video.play()` uses software decode only
- **Pixel output:** Floating-point rounding differences produce statistically distinct canvas hashes — anti-bot systems maintain databases of known SwiftShader vs. real-GPU hashes

---

## 7. Can Headless Chrome Be Made Indistinguishable From Headed Chrome?

### Techniques That Exist

| Technique | What It Patches | Limitations |
|---|---|---|
| `puppeteer-extra-plugin-stealth` (17+ modules) | `webdriver`, `plugins`, `chrome.runtime`, `languages`, `webgl.vendor`, `userAgent`, etc. | Only covers JS layer; patches are themselves detectable via `toString()` signatures |
| `--disable-blink-features=AutomationControlled` | Removes `navigator.webdriver` | Single signal; doesn't fix plugins, chrome.runtime, etc. |
| Custom CDP overrides in Playwright/Puppeteer | `page.addInitScript()` to redefine navigator properties | Fails to propagate to Web Workers |
| Antidetect browsers (e.g., CloakBrowser, fingerprint-chromium) | Source-level Chromium patches for GPU, fonts, audio, canvas | Still runs on a VM/server with detectable hardware |
| Real GPU passthrough (NVIDIA, e.g. T4) | `--use-angle=vulkan --enable-features=Vulkan --enable-gpu` | Requires expensive GPU hardware; still needs Xvfb for display surface |
| xvfb-run + headed Chrome | Runs full headed Chrome with virtual display | Adds ~200ms overhead; still detectable via behavioral analysis |

### Fundamental Limits (Cannot Be Fully Spoofed)

1. **GPU rendering artifacts:** SwiftShader's pixel output differs from any physical GPU. Anti-bot systems maintain databases of GPU-specific rendering hashes. No software renderer produces output matching a real GPU's driver/hardware combination.

2. **GPU draw timing:** CPU-based SwiftShader is 10-100x slower than real GPU. Timing measurements via `performance.now()` reveal software rendering regardless of spoofed strings.

3. **TLS/JA4 fingerprint:** The TLS stack of the automation framework (Node.js, Python, etc.) differs from Chrome's native TLS. Real Chrome has specific cipher order, GREASE values, and ALPN negotiation. `chrome-headless-shell` uses the same TLS stack as Chrome, but tools like puppeteer-extra still expose non-Chrome TLS quirks.

4. **CDP artifacts:** The Chrome DevTools Protocol leaves observable side effects in the page. Detection scripts can detect `Runtime.enable` by watching `Error.stack` getter behavior.

5. **Worker thread inconsistency:** Stealth patches applied to the main window context often don't propagate to Web Workers. `self.navigator.webdriver` in a Worker may still be `true` even after main-thread patching.

6. **Font enumeration:** Server/Docker environments lack the hundreds of fonts a real desktop has. Font list size and available typefaces are a high-entropy signal.

7. **Media devices:** `enumerateDevices()` returns zero audio/video inputs in headless. No stealth patch can create fake media devices without OS-level drivers.

8. **Hardware concurrency / memory mismatch:** Servers typically expose 1-4 cores and 0.5-4 GB RAM. Spoofing `navigator.hardwareConcurrency` to "8" doesn't change the actual parallelism; timing analysis reveals the true core count.

9. **Behavioral analysis:** Headless browsers don't generate human-like mouse movements, scroll patterns, or keystroke dynamics. Services like Cloudflare, DataDome, and Akamai score behavior, not just fingerprint.

### Verdict

**Fully indistinguishable is not possible in 2026.** A sophisticated setup (real GPU + headed Chrome via Xvfb + full fingerprint patching + residential proxy) can pass basic 1st-party detection. But against ML-based anti-bot systems that correlate 100+ signals including GPU hashes, TLS fingerprints, and behavioral patterns, headless automation remains detectable.

---

## 8. Puppeteer `headless:false` + `xvfb-run` vs. Native Headless

### Setup

**xvfb-run approach:**
```bash
xvfb-run --server-args="-screen 0 1920x1080x24" chromium-browser
```
Orchestrated via Selenoid, Browserless, or custom Docker entrypoint.

### Comparison

| Signal | `--headless=new` (native) | `headless:false` + `xvfb-run` |
|---|---|---|
| **window.chrome.runtime** | Patched available | Natively available |
| **navigator.plugins** | Populated (same as headed) | Natively populated |
| **navigator.webdriver** | `true` (without patch) | `true` (without patch) |
| **User-Agent** | Can contain `HeadlessChrome` | Contains `Chrome` (no headless marker) |
| **WebGL renderer** | SwiftShader (without GPU) | Real GPU if available, or Mesa/SwiftShader |
| **Screen dimensions** | Default 800x600 | Set via xvfb args (1920x1080 shown above) |
| **Font availability** | Sparse (container/OS dependent) | Same as native headless (depends on image) |
| **Canvas fingerprint** | SwiftShader-specific hash | Real GPU hash (if GPU available) |
| **Performance overhead** | Low~Medium | Medium~High (~200ms per page, X overhead) |
| **X11/Wayland dependency** | Required on Linux | Required (xvfb provides virtual display) |
| **D-Bus dependency** | Required | Required (same) |
| **CDP leaks** | Same (both use CDP) | Same |
| **Color depth** | 24-bit (default) | 24-bit (configurable via xvfb) |

### Key Differences

1. **GPU access:** `xvfb-run` provides a virtual X display which Chrome can use for hardware GPU rendering if `--enable-gpu` and `--use-angle=vulkan` are passed. Native headless typically blocks GPU access unless explicitly enabled.

2. **Screen geometry realism:** xvfb-run lets you set arbitrary resolutions (e.g., 1920x1080). Native headless defaults to 800x600 (configurable via `--window-size` and `--screen-info` since Chrome 135+).

3. **`window.outerWidth` vs `window.innerWidth`:** In xvfb + headed mode, there's a real difference (browser chrome takes space). In native headless, they're identical. xvfb-run gives a more realistic geometry fingerprint.

4. **Browser chrome:** Headed mode under xvfb still renders browser UI elements (even though they're not visible), including toolbars, scrollbars, and the omnibox. Some JS APIs detect the presence/absence of browser UI.

5. **Dependency cost:** xvfb-run requires Xvfb, `fluxbox` or similar window manager, and additional X11 libraries (~200MB+ extra). Native headless has no display server dependency (on Linux, it still requires X11/Xvfb in practice for WebGL even with `--headless=new`).

### Recommendation for Stealth

**`headless:false` + `xvfb-run` + real GPU passthrough** is currently the most realistic automation setup. It produces:
- Real GPU WebGL strings
- Real GPU canvas fingerprints
- Realistic screen geometry with browser chrome
- No `HeadlessChrome` in UA
- `window.chrome.runtime` and plugins intact

**Cost:** Slower startup, more resource-heavy, requires GPU hardware in the cloud (NVIDIA T4, A10G, etc.), and still vulnerable to behavioral detection.

### For basic sites: Native `--headless=new` + `puppeteer-extra-plugin-stealth` often suffices.
### For advanced anti-bot (Cloudflare, DataDome, Akamai): `xvfb-run` + headed + GPU + residential proxy is the minimum viable setup.
### For truly indistinguishable automation: This does not exist in 2026. All automated browsers leak detectable signals at some layer.

---

## References

- Chromium Headless README: https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/headless/README.md
- Chrome Headless mode docs: https://developer.chrome.com/docs/chromium/headless
- Removing `--headless=old`: https://developer.chrome.com/blog/removing-headless-old-from-chrome
- chrome-headless-shell announcement: https://developer.chrome.com/blog/chrome-headless-shell
- Screen configuration with Headless: https://developer.chrome.com/blog/screen-configuration-with-chrome-headless
- SwiftShader docs: https://chromium.googlesource.com/chromium/src/+/main/docs/gpu/swiftshader.md
- Using GPU hardware in Headless: https://chromium.googlesource.com/chromium/src/+/main/docs/gpu/using-gpu-hardware-in-headless-chrome.md
- Antoine Vastel: New headless Chrome fingerprint: https://arh.antoinevastel.com/bot%20detection/2023/02/19/new-headless-chrome.html
- CreepJS browser checker: https://creepjs.org/
- Device and Browser Info: https://deviceandbrowserinfo.com/
