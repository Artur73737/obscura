# Obscura Stealth — Chrome Fingerprint Reference

> **Domanda:** Possiamo replicare letteralmente il fingerprint di Google Chrome, non solo approssimarlo?
>
> **Risposta:** Sì, possiamo replicare **ogni singola superficie** che Chrome espone.
> Obscura è un browser engine basato su V8/Blink (via deno_core) — abbiamo lo stesso JS engine di Chrome.
> Dobbiamo solo assicurarci che ogni API JS, ogni header HTTP, ogni parametro TLS,
> e ogni comportamento del runtime corrisponda esattamente a Chrome.
>
> Questo documento elenca TUTTE le superfici di fingerprinting conosciute e i valori esatti di Chrome 136+ su Windows.

---

## Indice

1. [Navigator Properties](#1-navigator-properties)
2. [Window & Chrome Object](#2-window--chrome-object)
3. [Screen & Viewport](#3-screen--viewport)
4. [WebGL & Canvas](#4-webgl--canvas)
5. [AudioContext](#5-audiocontext)
6. [HTTP Headers](#6-http-headers)
7. [TLS & HTTP/2 Fingerprint](#7-tls--http2-fingerprint)
8. [Timing & Performance](#8-timing--performance)
9. [Storage & Quota](#9-storage--quota)
10. [Media & Codecs](#10-media--codecs)
11. [CSS Feature Detection](#11-css-feature-detection)
12. [Font Fingerprinting](#12-font-fingerprinting)
13. [Permissions API](#13-permissions-api)
14. [Workers & Cross-Context Consistency](#14-workers--cross-context-consistency)
15. [Property Descriptors & toString()](#15-property-descriptors--tostring)
16. [Error Stacks](#16-error-stacks)
17. [WebAssembly](#17-webassembly)
18. [WebGPU](#18-webgpu)
19. [Navigator Prototype Checklist](#19-navigator-prototype-checklist)
20. [Implementation Plan per Obscura](#20-implementation-plan-per-obscura)

---

## 1. Navigator Properties

### `navigator.webdriver`

| Stato | Valore |
|---|---|
| Chrome normale (nessun flag automation) | `false` |
| Chrome con `--enable-automation` | `true` |
| Chrome headless (nuovo) | `true` (se non disabilitato) |
| Chrome con `--disable-blink-features=AutomationControlled` | `false` |

**Property descriptor:**
- Vive su `Navigator.prototype`, NON su `navigator` stesso
- `Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')` → `{ get: [native code], set: undefined, enumerable: true, configurable: true }`
- `Function.prototype.toString.call(getter)` → `"function get webdriver() { [native code] }"`
- In Chrome **non-automated**: `navigator.webdriver === false` (sempre presente, mai `undefined`)
- `Object.getOwnPropertyDescriptor(navigator, 'webdriver')` → `undefined` (non è own-property!)

**Come replicare in Obscura:**
```javascript
// Opzione 1 (migliore): via flag interno C++/Rust
// Obscura deve avere un flag interno "automation_controlled" che di default è false
// e viene attivato solo se esplicitamente richiesto

// Opzione 2 (JS, meno sicura):
Object.defineProperty(Navigator.prototype, 'webdriver', {
    get: function() { return false; },
    configurable: true,
    enumerable: true
});
```

### `navigator.plugins` (PluginArray)

Chrome 136+ (PDF viewer attivo): **5 plugins** sempre con lo stesso filename/description:

| Index | Name | Filename | Description |
|---|---|---|---|
| 0 | `PDF Viewer` | `internal-pdf-viewer` | `Portable Document Format` |
| 1 | `Chrome PDF Viewer` | `internal-pdf-viewer` | `Portable Document Format` |
| 2 | `Chromium PDF Viewer` | `internal-pdf-viewer` | `Portable Document Format` |
| 3 | `Microsoft Edge PDF Viewer` | `internal-pdf-viewer` | `Portable Document Format` |
| 4 | `WebKit built-in PDF` | `internal-pdf-viewer` | `Portable Document Format` |

**2 MIME types** (condivisi da tutti i plugin):
- `application/pdf` → suffixes `pdf`, desc `Portable Document Format`
- `text/pdf` → suffixes `pdf`, desc `Portable Document Format`

**Invarianti critiche (spesso sbagliate dalle stealth):**
- `navigator.plugins` NON è un Array, è un `PluginArray` con `item()`, `namedItem()`, `refresh()`
- `plugins[0]` e `plugins.namedItem("PDF Viewer")` devono essere lo **stesso oggetto** (reference identity)
- `plugins[i].item(j).enabledPlugin === plugins[i]` per ogni i, j (bidirectional cross-reference)
- `refresh()` deve essere presente e **writable**: `navigator.plugins.refresh = 'test'` deve funzionare
- `navigator.plugins instanceof PluginArray` deve essere `true`
- Named access: `navigator.plugins['PDF Viewer']` deve funzionare
- Gli oggetti devono essere creati con `Object.create(Plugin.prototype)` non `{}`
- `for...of` e `for...in` devono dare gli stessi risultati

### `navigator.mimeTypes` (MimeTypeArray)

| Index | Type | Suffixes | Description | enabledPlugin |
|---|---|---|---|---|
| 0 | `application/pdf` | `pdf` | `Portable Document Format` | → `plugins[0]` |
| 1 | `text/pdf` | `pdf` | `Portable Document Format` | → `plugins[0]` |

### `navigator.languages` / `navigator.language`

```
navigator.language    → "it-IT"
navigator.languages   → ["it-IT", "it", "en-US", "en"]
```

- Il primo elemento di `languages` è sempre === `language`
- Chrome 136+ ha ridotto `Accept-Language` header a un solo tag, ma `navigator.languages` rimane invariato
- In Incognito, `navigator.languages` è ridotto a un singolo elemento
- L'array deve essere **non-freezato** (ogni accesso può restituire una nuova copia)
- Deve matchare l'`Accept-Language` header per coerenza cross-canale

### `navigator.userAgent` / appVersion / platform / vendor

| Property | Valore Chrome 136+ Windows |
|---|---|
| `userAgent` | `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36` |
| `appVersion` | `5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36` |
| `platform` | `Win32` (frozen, anche su Windows 64-bit) |
| `vendor` | `Google Inc.` |
| `appName` | `Netscape` |
| `appCodeName` | `Mozilla` |
| `product` | `Gecko` |
| `productSub` | `20030107` |

### `navigator.userAgentData` (Client Hints)

**Low-entropy (sempre disponibili):**
```javascript
navigator.userAgentData.brands
// → [{brand: "Not/A,Brand", version: "99"},
//     {brand: "Chromium", version: "136"},
//     {brand: "Google Chrome", version: "136"}]

navigator.userAgentData.mobile   → false
navigator.userAgentData.platform → "Windows"
```

**High-entropy (getHighEntropyValues):**
```javascript
await navigator.userAgentData.getHighEntropyValues([
  'architecture', 'bitness', 'model',
  'platformVersion', 'fullVersionList',
  'wow64', 'formFactor'
])
// → {
//     architecture: "x86",
//     bitness: "64",
//     model: "",
//     platformVersion: "15.0.0",  // Win11 24H2+
//     fullVersionList: [
//       {brand: "Not/A,Brand", version: "99.0.0.0"},
//       {brand: "Chromium", version: "136.0.7103.114"},
//       {brand: "Google Chrome", version: "136.0.7103.114"}
//     ],
//     wow64: false,
//     formFactor: []
//   }
```

**PlatformVersion mapping Windows:**
- Win10 → `"10.0.0"`
- Win11 23H2 → `"14.0.0"`
- Win11 24H2+ → `"15.0.0"`

### Altre navigator properties

| Property | Chrome 136+ Valore |
|---|---|
| `hardwareConcurrency` | `8` (core logici reali) |
| `deviceMemory` | `8` (GB, arrotondato per difetto alla potenza di 2, cap a 8) |
| `pdfViewerEnabled` | `true` |
| `cookieEnabled` | `true` |
| `doNotTrack` | `null` (default) |
| `maxTouchPoints` | `0` (desktop senza touch) |
| `onLine` | `true` |
| `webdriver` | `false` |
| `vendorSub` | `""` |
| `oscpu` | non esiste (è Firefox) |
| `buildID` | non esiste (è Firefox) |
| `globalPrivacyControl` | non esiste in Chrome di default |

### `navigator.connection` (NetworkInformation)

```javascript
navigator.connection.effectiveType  // "4g" (desktop su ethernet)
navigator.connection.downlink       // 10.0 (Mbps)
navigator.connection.rtt            // 50 (ms)
navigator.connection.saveData       // false
navigator.connection.type           // "wifi" o "ethernet"
```

- **Chromium-only** - Firefox/Safari non lo hanno. La sua presenza identifica Chrome.
- Valori bucketed intenzionalmente, ma la combinazione è fingerprint.

---

## 2. Window & Chrome Object

### `window.chrome`

In Chrome 136+ (nuovo headless incluso), `window.chrome` è un oggetto completo. In vecchio headless (pre-112) era missing.

```javascript
window.chrome
// → {
//     app: { ... },
//     runtime: { ... },
//     csi: function(),
//     loadTimes: function(),
//     webstore: undefined  // deprecato da Chrome 71
//   }
```

### `chrome.runtime`

```javascript
// In page context (NON extension):
chrome.runtime.id              // undefined
chrome.runtime.connect()       // function - restituisce dead Port
chrome.runtime.sendMessage()   // function - valida extension ID
chrome.runtime.onConnect       // EventTarget-like
chrome.runtime.onMessage       // EventTarget-like

// Proprietà importanti:
// - sendMessage.toString() → "function () { [native code] }"
// - connect.toString() → "function () { [native code] }"
// - onConnect instanceof EventTarget → true
// - Le funzioni NON hanno proprietà .prototype (native bound functions)
```

### `chrome.csi()`

```javascript
chrome.csi()
// → {
//     onloadT: 1513186742842,  // epoch ms del load event
//     pageT: 1513186741847,    // offset dal navigation start
//     startE: 1513186741847,   // start time
//     tran: 15                 // page transition type
//   }
```

Deprecato da Chrome 64 ma ancora presente e funzionante nel 2026.

### `chrome.loadTimes()`

```javascript
chrome.loadTimes()
// → {
//     requestTime: 1513186741.847,          // epoch seconds
//     startLoadTime: 1513186741.847,
//     commitLoadTime: 1513186742.637,
//     finishDocumentLoadTime: 1513186742.842,
//     finishLoadTime: 1513186743.582,
//     firstPaintTime: 1513186742.829,
//     firstPaintAfterLoadTime: 1513186742.829,  // 0 in headless old!
//     navigationType: "Reload",
//     wasFetchedViaSpdy: true,
//     wasNpnNegotiated: true,
//     npnNegotiatedProtocol: "h2",
//     wasAlternateProtocolAvailable: false,
//     connectionInfo: "h2"
//   }
```

**Segnale headless detection:** `firstPaintAfterLoadTime === 0` in old headless (nessun GPU pipeline). Nuovo headless restituisce valore non-zero.

---

## 3. Screen & Viewport

### Screen properties

```javascript
screen.width            // 1920
screen.height           // 1080
screen.availWidth       // 1920
screen.availHeight      // 1040 (1080 - 40 taskbar)
screen.colorDepth       // 24 (o 32 su Windows con driver specifici)
screen.pixelDepth       // 24 (sempre === colorDepth per spec)
screen.orientation.type // "landscape-primary"
```

### Window geometry

```javascript
window.innerWidth       // 1920 (viewport)
window.innerHeight      // 974
window.outerWidth       // 1936 (incluso chrome)
window.outerHeight      // 1056 (incluso chrome)
window.screenX          // 0 (posizione sinistra)
window.screenY          // 0 (posizione top)
window.devicePixelRatio // 1.0 (o 1.25, 1.5, 2.0 su HiDPI)
```

**Invariante critica:** `outerWidth >= innerWidth` e `outerHeight >= innerHeight`.
In headless `outerHeight - innerHeight === 0` → nessun chrome → segnale detection.

---

## 4. WebGL & Canvas

### WebGL Renderer / Vendor (UNMASKED)

```javascript
const ext = gl.getExtension('WEBGL_debug_renderer_info');
const vendor = gl.getParameter(ext.UNMASKED_VENDOR_WEBGL);
const renderer = gl.getParameter(ext.UNMASKED_RENDERER_WEBGL);
```

| GPU | Vendor | Renderer |
|---|---|---|
| NVIDIA RTX 3080 (Windows) | `Google Inc. (NVIDIA)` | `ANGLE (NVIDIA, NVIDIA GeForce RTX 3080, D3D11)` |
| Intel UHD 620 (Windows) | `Google Inc. (Intel)` | `ANGLE (Intel, Intel(R) UHD Graphics 620, D3D11)` |
| Apple M4 Pro (macOS) | `Google Inc. (Apple)` | `ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Pro, Unspecified Version)` |
| Headless (SwiftShader) | `Google Inc. (Google)` | `Google SwiftShader` |

Su Windows, ANGLE traduce WebGL → Direct3D, quindi il vendor include sempre `"Google Inc."`.
Mancanza di questo prefisso su Windows è sospetta.

### WebGL Parameters (~50 parametri fingerprintabili)

```javascript
gl.getParameter(gl.MAX_TEXTURE_SIZE);           // 16384 (Intel), 32768 (NVIDIA)
gl.getParameter(gl.MAX_CUBE_MAP_TEXTURE_SIZE);  // 16384
gl.getParameter(gl.MAX_RENDERBUFFER_SIZE);      // 16384/32768
gl.getParameter(gl.MAX_VIEWPORT_DIMS);          // [32768, 32768]
gl.getParameter(gl.MAX_VERTEX_ATTRIBS);         // 16 (quasi sempre)
gl.getParameter(gl.MAX_VERTEX_UNIFORM_VECTORS); // 4096
gl.getParameter(gl.MAX_FRAGMENT_UNIFORM_VECTORS);// 4096 o 1024
gl.getParameter(gl.MAX_VARYING_VECTORS);        // 30 o 16
gl.getParameter(gl.ALIASED_LINE_WIDTH_RANGE);   // [1,1] (Intel) o [1,8191] (NVIDIA)
gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE);   // [1, 1024]
gl.getParameter(gl.RED_BITS);   // 8
gl.getParameter(gl.GREEN_BITS); // 8
gl.getParameter(gl.BLUE_BITS);  // 8
gl.getParameter(gl.DEPTH_BITS);  // 24
gl.getParameter(gl.STENCIL_BITS); // 8
```

### WebGL2-Only Parameters

```javascript
gl.getParameter(gl.MAX_3D_TEXTURE_SIZE);         // 2048-16384
gl.getParameter(gl.MAX_ARRAY_TEXTURE_LAYERS);     // 2048-16384
gl.getParameter(gl.MAX_COLOR_ATTACHMENTS);        // 4-8
gl.getParameter(gl.MAX_DRAW_BUFFERS);             // 4-8
gl.getParameter(gl.MAX_SAMPLES);                  // 4-8
gl.getParameter(gl.MAX_UNIFORM_BUFFER_BINDINGS);  // 24-84
gl.getParameter(gl.MAX_UNIFORM_BLOCK_SIZE);       // 16384-65536
```

### WebGL Extensions

Chrome supporta ~35+ estensioni WebGL1 e ~25+ estensioni WebGL2.
Lista completa da replicare: vedi `getSupportedExtensions()` di Chrome.

### Canvas Fingerprinting

Il canvas fingerprint standard (FingerprintJS):
```javascript
const canvas = document.createElement('canvas');
canvas.width = 256;
canvas.height = 256;
const ctx = canvas.getContext('2d');
ctx.textBaseline = 'top';
ctx.font = '14px Arial';
ctx.fillStyle = '#f60';
ctx.fillRect(125, 1, 62, 20);
ctx.fillStyle = '#069';
ctx.fillText('Cwm fjordbank glyph', 2, 15);
ctx.fillStyle = 'rgba(102, 204, 0, 0.7)';
ctx.fillText('Cwm fjordbank glyph', 4, 17);
return canvas.toDataURL();  // base64 PNG
```

**Chrome NON ha protezioni anti-fingerprinting per canvas.** Nessun rumore, nessuna randomizzazione.
Il fingerprint canvas dipende da: GPU, driver, OS, font installati, versione Skia.

Entropia canvas: ~16 bit. Entropia WebGL + canvas combinati: >33 bit (unicità globale).

---

## 5. AudioContext

L'audio fingerprint usa `OfflineAudioContext`:
```javascript
const ctx = new OfflineAudioContext(1, 5000, 44100);
const osc = ctx.createOscillator();
osc.type = 'triangle';
osc.frequency.value = 10000;
const comp = ctx.createDynamicsCompressor();
comp.threshold.value = -50;
comp.knee.value = 40;
comp.ratio.value = 12;
comp.attack.value = 0;
comp.release.value = 0.25;
osc.connect(comp);
comp.connect(ctx.destination);
osc.start(0);
const buffer = await ctx.startRendering();
const data = buffer.getChannelData(0);
// Hash dei samples 4500-5000
let hash = 0;
for (let i = 4500; i < 5000; i++) hash += Math.abs(data[i]);
```

**Valori tipici Chrome 136 su Windows:**
- Hash: `124.04347527516074`
- Chrome su macOS differisce alla ~6a cifra decimale (Apple Accelerate framework)
- Firefox: hash Drammaticamente diverso (`35.73833402246237`)

---

## 6. HTTP Headers

### Chrome 136+ Request Headers (navigazione, ordine esatto)

```
:method: GET
:authority: www.example.com
:scheme: https
:path: /
sec-ch-ua: "Chromium";v="136", "Google Chrome";v="136", "Not/A.Brand";v="99"
sec-ch-ua-mobile: ?0
sec-ch-ua-platform: "Windows"
sec-ch-ua-platform-version: "15.0.0"
sec-ch-ua-arch: "x86"
sec-ch-ua-bitness: "64"
sec-ch-ua-wow64: ?0
sec-ch-ua-model: ""
sec-ch-ua-full-version: "136.0.7103.114"
upgrade-insecure-requests: 1
user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36
accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8
sec-fetch-site: none
sec-fetch-mode: navigate
sec-fetch-user: ?1
sec-fetch-dest: document
accept-encoding: gzip, deflate, br, zstd
accept-language: it-IT,it;q=0.9,en-US;q=0.8,en;q=0.7
priority: u=0, i
```

### Accept-Encoding

Chrome 136+: `gzip, deflate, br, zstd` (zstd aggiunto in Chrome 123)

### Accept-Language

Chrome 136+ ha ridotto a un solo tag: `it-IT,it;q=0.9` (ma invariato in JS).

### Sec-CH-UA (User-Agent Client Hints)

I valori esatti cambiano ad ogni versione di Chrome. Il GREASE brand `"Not/A.Brand"` varia nome e versione periodicamente.

### Cache-Control

Chrome NON invia `Cache-Control` sulle richieste di navigazione iniziali (solo su reload: `no-cache`).
Sulle risorse (immagini, JS, CSS) rispetta il `Cache-Control` del server.

---

## 7. TLS & HTTP/2 Fingerprint

### Chrome 136 Cipher Suites (BoringSSL, Windows)

Dopo rimozione GREASE:
```
TLS_AES_128_GCM_SHA256          (4865)
TLS_AES_256_GCM_SHA384          (4866)
TLS_CHACHA20_POLY1305_SHA256   (4867)
TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256   (49195)
TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256     (49199)
TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384   (49196)
TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384     (49200)
TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256   (52393)
TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256     (52392)
TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA   (49171)
TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA     (49172)
TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA   (156)
TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA     (157)
TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA     (47)   // TLS 1.0
TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA     (53)   // TLS 1.0
```

**JA4 fingerprint:** `t13d1516h2_8daaf6152771_d8a2da3f94cd`

### TLS Extensions (Chrome 136, ~16 dopo GREASE)

Ordine randomizzato per connessione (JA3 è rotto per Chrome).

```
0   server_name (SNI)
5   status_request (OCSP)
10  supported_groups
11  ec_point_formats
13  signature_algorithms
16  application_layer_protocol_negotiation (ALPN)
18  signed_certificate_timestamp
23  extended_master_secret
27  compress_certificate (Brotli)
35  session_ticket
43  supported_versions
45  key_share
51  key_share (post-quantum hybrid: X25519MLKEM768)
65281  renegotiation_info
17513  application_settings (ALPS)
21  padding
```

### Elliptic Curves / Named Groups

```
x25519 (29)
secp256r1 (23)
secp384r1 (24)
x25519mlkem768 (4588/0x11ec)  // post-quantum hybrid
```

### HTTP/2 SETTINGS Frame (Akamai fingerprint)

```
1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p
  ├ HEADER_TABLE_SIZE=65536
  ├ ENABLE_PUSH=0
  ├ INITIAL_WINDOW_SIZE=6291456 (6 MB)
  ├ MAX_HEADER_LIST_SIZE=262144 (256 KB)
  └ WINDOW_UPDATE=15663105
    PRIORITY=0 (deprecated, Extensible Priorities RFC 9218)
    Pseudo-header order: :method, :authority, :scheme, :path
```

Chrome 130+ aggiunge SETTINGS `31386=3814263365` (vendor extension non IANA).

---

## 8. Timing & Performance

### `performance.now()` precision

| Contesto | Precisione |
|---|---|
| Pagina normale | **~100 µs (0.1 ms)** |
| Cross-origin isolated (COOP+COEP) | **~5 µs** |

### `performance.memory` (Chrome-only)

```javascript
performance.memory
// → {
//     jsHeapSizeLimit: 4294705152,  // 4 GB
//     totalJSHeapSize: 39973671,
//     usedJSHeapSize: 39127515
//   }
```

`jsHeapSizeLimit` è 4 GB su macchine ≤32 GB RAM, 8 GB su >32 GB RAM.

### Navigation Timing

```javascript
performance.getEntriesByType('navigation')[0]
// → {
//     type: "navigate",
//     redirectCount: 0,
//     nextHopProtocol: "h2",
//     transferSize: 12345,     // 0 se da cache
//     encodedBodySize: 12345,
//     decodedBodySize: 45678,
//     renderBlockingStatus: "non-blocking",
//     ...
//   }
```

### Paint Timing

```javascript
performance.getEntriesByType('paint')
// → [
//     { name: "first-paint", startTime: 234.5 },
//     { name: "first-contentful-paint", startTime: 234.5 }
//   ]
```

---

## 9. Storage & Quota

### `navigator.storage.estimate()`

```javascript
await navigator.storage.estimate()
// → {
//     quota: 10737418240,  // ~10 GB (Chrome normale)
//     usage: 42240,        // bytes usati dall'origine
//     usageDetails: {
//       indexedDB: 32768,
//       cacheapi: 9472
//     }
//   }
```

- Chrome normale: ~10 GB (60% del disco, pooled)
- Chrome Incognito: ~4 GB (RAM-backed) → **segnale incognito detection**
- `quota < 120MB` → probabilmente private browsing

### Service Worker

```javascript
navigator.serviceWorker   // ServiceWorkerContainer
navigator.serviceWorker.controller  // ServiceWorker | null
```

- Solo su HTTPS
- `controller` è `null` se nessun SW attivo

---

## 10. Media & Codecs

### `canPlayType()` codecs supportati

| Codec string | Result |
|---|---|
| `video/mp4; codecs="avc1.4D401E"` | `probably` |
| `video/webm; codecs="vp9"` | `probably` |
| `video/webm; codecs="av01.0.00M.08"` | `probably` (se AV1 abilitato) |
| `video/mp4; codecs="hvc1.1.6.L93.B0"` | varies per piattaforma (HEVC) |
| `audio/mpeg` | `probably` |
| `audio/webm; codecs="opus"` | `probably` |
| `audio/wav` | `probably` |

### `MediaCapabilities.decodingInfo()`

```javascript
await navigator.mediaCapabilities.decodingInfo({
  type: 'file',
  video: { contentType: 'video/mp4;codecs=avc1.640028', width: 1920, height: 1080, bitrate: 5000000, framerate: 30 },
  audio: { contentType: 'audio/mp4;codecs=mp4a.40.2', channels: 2, samplerate: 48000 }
})
// → { supported: true, smooth: true, powerEfficient: true }
```

### `navigator.mediaDevices.enumerateDevices()`

Prima del permesso `getUserMedia`:
```javascript
// → [{ deviceId: "", kind: "audioinput", label: "", groupId: "" },
//     { deviceId: "", kind: "audiooutput", label: "", groupId: "" },
//     { deviceId: "", kind: "videoinput", label: "", groupId: "" }]
```

Senza permesso, `deviceId` e `label` sono stringhe vuote.

---

## 11. CSS Feature Detection

Queste query `CSS.supports()` distinguono Chrome da altri browser:

```javascript
CSS.supports('selector(::-webkit-scrollbar)')   // true (Chrome + Safari, false Firefox)
CSS.supports('scrollbar-color: auto')           // true (Chrome 121+ + Firefox)
CSS.supports('backdrop-filter: blur(1px)')      // true (Chrome 76+)
CSS.supports('color-scheme: dark')              // true (Chrome 81+)
CSS.supports('selector(:has(*))')               // true (Chrome 105+)
CSS.supports('color: color(display-p3 1 0 0)') // true se schermo wide-gamut
```

### Media queries fingerprint

```javascript
matchMedia('(prefers-color-scheme: dark)').matches   // true/false
matchMedia('(prefers-reduced-motion: reduce)').matches // true/false
matchMedia('(prefers-contrast: more)').matches         // true/false
matchMedia('(color-gamut: p3)').matches                // true se wide-gamut
matchMedia('(pointer: fine)').matches                   // true se mouse
```

---

## 12. Font Fingerprinting

Chrome **NON** ha protezioni built-in per font fingerprinting. Espone tutti i font installati.

```javascript
// Canvas measureText() font probing (senza permesso)
const ctx = document.createElement('canvas').getContext('2d');
const testString = 'mmmmmmmmmmlli';
ctx.font = '72px "Calibri", monospace';
const probeWidth = ctx.measureText(testString).width;
ctx.font = '72px monospace';
const baseline = ctx.measureText(testString).width;
// Se probeWidth !== baseline → font installato
```

Windows 11 ha ~100-120 famiglie di font di base (pulito). Con Office 365 → 300-600+ famiglie.

---

## 13. Permissions API

```javascript
await navigator.permissions.query({ name: 'geolocation' })
// → { state: 'prompt' }  (default, mai chiesto)
await navigator.permissions.query({ name: 'notifications' })
// → { state: 'prompt' }
await navigator.permissions.query({ name: 'background-sync' })
// → { state: 'granted' }  (sempre granted in Chrome)
await navigator.permissions.query({ name: 'clipboard-write' })
// → { state: 'granted' }  (default)
await navigator.permissions.query({ name: 'clipboard-read' })
// → { state: 'prompt' }
```

**Chrome-only permissions:**
```javascript
'ambient-light-sensor', 'accelerometer', 'gyroscope', 'magnetometer',
'screen-wake-lock', 'clipboard-read', 'clipboard-write', 'display-capture',
'local-fonts', 'window-management'
```

Firefox/Safari lanciano eccezione per questi → la loro presenza identifica Chrome.

---

## 14. Workers & Cross-Context Consistency

**Regola d'oro:** Un vero browser restituisce gli **stessi valori** in tutti i contesti:
- Main thread
- Web Worker (`new Worker()`)
- SharedWorker
- Service Worker
- iframe

Proprietà da verificare cross-contesto:
- `navigator.userAgent` / `navigator.platform`
- `navigator.hardwareConcurrency` / `navigator.deviceMemory`
- `navigator.userAgentData`
- `navigator.language` / `navigator.languages`
- Canvas hash (via `OffscreenCanvas` in worker)
- WebGL renderer (via `OffscreenCanvas` in worker)

Qualsiasi disaccordo tra contesti → manomissione rilevata.

---

## 15. Property Descriptors & toString()

I detector sofisticati non leggono solo i valori, ispezionano i **descriptor**:

```javascript
// 1. Dov'è definita la proprietà?
Object.getOwnPropertyDescriptor(navigator, 'webdriver')
// → undefined (vive su prototype, non su instance)

Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')
// → { get: [native code], set: undefined, enumerable: true, configurable: true }

// 2. toString() del getter
Function.prototype.toString.call(descriptor.get)
// → "function get webdriver() { [native code] }"  // se nativo
// → "function get webdriver() { return undefined; }"  // se patchato JS
// → "() => undefined"  // se arrow function

// 3. Instanceof check
navigator.plugins instanceof PluginArray  // true

// 4. Cross-realm check (iframe)
iframe.contentWindow.navigator.webdriver  // deve essere === parent

// 5. Prototype chain
Object.getPrototypeOf(navigator) === Navigator.prototype  // true

// 6. Enumerabilità
Object.keys(navigator)  // non deve contenere le proprietà prototype

// 7. Configurabilità
delete navigator.webdriver  // NON deve funzionare (è su prototype)
// se funziona → era own-property → patchato
```

### toString() checks specifici (Kasada, Akamai)

```javascript
HTMLCanvasElement.prototype.getContext.toString()
// → "function getContext() { [native code] }"

WebGLRenderingContext.prototype.getParameter.toString()
// → "function getParameter() { [native code] }"

navigator.mediaDevices.enumerateDevices.toString()
// → "function enumerateDevices() { [native code] }"

// 20-method check:
const methods = [
  'navigator.webdriver',
  'chrome.csi', 'chrome.loadTimes', 'chrome.runtime.sendMessage', 'chrome.runtime.connect',
  'HTMLCanvasElement.prototype.getContext',
  'WebGLRenderingContext.prototype.getParameter',
  'navigator.mediaDevices.enumerateDevices',
  'Navigator.prototype.webdriver',
  'Navigator.prototype.languages',
  'Navigator.prototype.plugins',
  'Navigator.prototype.mimeTypes',
  'PluginArray.prototype.item', 'PluginArray.prototype.namedItem',
  'MimeTypeArray.prototype.item', 'MimeTypeArray.prototype.namedItem',
  'Document.prototype.querySelector', 'Document.prototype.querySelectorAll',
  'Function.prototype.toString',
  'Object.getOwnPropertyDescriptor', 'Object.defineProperty'
];
// Ognuno deve tornare [native code]
```

---

## 16. Error Stacks

V8 stack trace format:
```
ErrorType: message
    at FunctionName (file.js:line:col)
    at new Constructor (file.js:line:col)
    at Object.method [as alias] (file.js:line:col)
    at async asyncFunction (file.js:line:col)
```

**Chrome-specific:**
- `Error.captureStackTrace(error, constructorOpt)` → **V8-only**, non in Firefox/Safari
- `Error.stackTraceLimit` → **V8-only**, default 10
- `Error.prepareStackTrace(error, structuredStackTrace)` → **V8-only**
- Linee iniziano con `at ` (Chrome) vs `@` (Safari) vs senza prefisso (Firefox)
- Frame async marcati `at async` in V8 (da V8 7.3, Chrome 73)
- Column numbers: **1-based** in V8

---

## 17. WebAssembly

```javascript
typeof WebAssembly  // "object" (Chrome 57+)

// SIMD support detection
WebAssembly.validate(new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123,
  3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 17, 253, 15, 11
]))  // true se SIMD supportato
```

**Chrome features WASM supportate (2026):**
MVP, Bulk Memory, Reference Types, SIMD, Relaxed SIMD, Threads (SAB con COOP+COEP), Tail Call, Exception Handling, Sign Extensions, Non-trapping Float-to-Int, Multi-value, Mutable Globals, BigInt, GC, Typed Function References, JS String Builtins, Branch Hinting

---

## 18. WebGPU

```javascript
const adapter = await navigator.gpu.requestAdapter();
const info = adapter.info;
// → { vendor: "nvidia", architecture: "ada", device: "AD102",
//     description: "NVIDIA GeForce RTX 4090",
//     type: "discrete GPU", backend: "Vulkan" }

const limits = adapter.limits;
// → ~40 parametri numerici (maxTextureDimension2D, maxBindGroups, etc.)

const features = adapter.features;
// → Set di feature: depth-clip-control, texture-compression-bc, shader-f16, etc.
```

WebGPU disponibile in Chrome 113+. Richiede HTTPS.

---

## 19. Navigator Prototype Checklist

Tutte le proprietà che Chrome espone su `Navigator.prototype`.
Ogni proprietà è un getter `{ get: [native code], set: undefined, enumerable: true, configurable: true }`.

**Standard (tutti i browser):**
```
clipboard, connection, cookieEnabled, credentials, geolocation,
hardwareConcurrency, language, languages, locks, maxTouchPoints,
mediaCapabilities, mediaDevices, mediaSession, onLine,
permissions, platform, plugins, product, serviceWorker,
storage, userAgent, vendor, webdriver
```

**Chrome-specific / non-standard:**
```
bluetooth, deviceMemory, gpu, hid, ink, keyboard,
pdfViewerEnabled, scheduling, serial, usb, userActivation,
userAgentData, virtualKeyboard, wakeLock, windowControlsOverlay, xr
```

**Assenti in Chrome (presenti in altri browser):**
```
buildID (Firefox), oscpu (Firefox), globalPrivacyControl (alcuni),
standalone (Safari iOS), contacts (Safari)
```

---

## 20. Implementation Plan per Obscura

### Obscura può replicare letteralmente Chrome perché:
- Usa V8 (stesso JS engine)
- Ha controllo completo su ogni API JS (via bootstrap.js e ops.rs)
- Ha controllo completo sugli header HTTP (via `wreq_client` o `client.rs`)
- Può implementare TLS con cipher suite identiche a Chrome (BoringSSL con build feature `stealth`)
- Può controllare property descriptors, error stacks, timing

### Cosa serve in Rust:

#### A. Modulo `fingerprint` (nuovo crate o modulo in `obscura-js`)

```rust
// Valori correnti di Chrome 136+
pub struct ChromeFingerprint {
    pub version: String,         // "136.0.0.0"
    pub full_version: String,    // "136.0.7103.114"
    pub platform: String,        // "Windows"
    pub platform_version: String, // "15.0.0"
    pub architecture: String,    // "x86"
    pub bitness: String,         // "64"
    pub wow64: bool,            // false
    pub ua_brands: Vec<Brand>,  // [{Chromium,136}, {Google Chrome,136}, {Not/A.Brand,99}]
}
```

#### B. Script JS di bootstrap (`js/bootstrap.js`)

Iniettare PRIMA di qualsiasi pagina web:
1. `Navigator.prototype.webdriver` getter → `false`
2. `Navigator.prototype.plugins` → PluginArray con 5 entries PDF
3. `Navigator.prototype.mimeTypes` → MimeTypeArray con 2 entries
4. `Navigator.prototype.languages` → array lingue
5. `Navigator.prototype.vendor` → `"Google Inc."`
6. `Navigator.prototype.pdfViewerEnabled` → `true`
7. `Navigator.prototype.deviceMemory` → `8`
8. `Navigator.prototype.hardwareConcurrency` → `8`
9. `Navigator.prototype.connection` → NetworkInformation object
10. `Navigator.prototype.userAgentData` → NavigatorUAData
11. `window.chrome` → oggetto completo con `runtime`, `csi`, `loadTimes`, `app`
12. `chrome.runtime` → `connect`, `sendMessage`, `onConnect`, `onMessage`
13. `chrome.runtime.connect.toString()` → `[native code]`
14. `chrome.loadTimes()` → funzione che restituisce timing object
15. `chrome.csi()` → funzione che restituisce timing object
16. Fix Error stack format (V8 già corretto di default)
17. WebGL renderer → specificare GPU string realistica
18. WebGL parameters → valori realistici per GPU dichiarata
19. Screen properties → dimensioni realistiche
20. `performance.memory` → valori realistici

#### C. TLS/HTTP client (`obscura-net`)

Con feature `stealth`:
- TLS cipher suite ordine identico a Chrome (BoringSSL)
- TLS extensions order randomizzato per connessione
- HTTP/2 SETTINGS frame identico a Chrome
- HTTP headers ordine identico a Chrome
- Sec-CH-UA headers aggiornati alla versione Chrome corrente
- Accept-Encoding: `gzip, deflate, br, zstd`
- HTTP/3 (QUIC) support

#### D. Mantenimento

- Aggiornare versione Chrome a ogni release
- I valori `sec-ch-ua` cambiano a ogni versione Chrome
- I cipher suites TLS possono cambiare
- I WebGL renderer strings sono statici (dipendono dalla GPU, non dalla versione Chrome)
- Nuove API JS vengono aggiunte da Chrome periodicamente

### NOTE: cosa NON possiamo replicare (limiti fondamentali)

1. **TLS fingerprint (JA4)**: BoringSSL ha cipher suite diverse da rustls. Con `stealth` feature usiamo `wreq` (basato su `curl`/`libcurl` con BoringSSL) → JA4 matcha Chrome.
2. **TCP/IP stack fingerprint**: Dipende dall'OS host, non dal browser. Su Windows, il TCP SYN packet è identico a Chrome su Windows.
3. **GPU hardware fingerprint**: Dipende dalla GPU fisica. Obscura usa la GPU del sistema → WebGL renderer sarà quello reale.
4. **Font fingerprint**: Dipende dai font installati. Su Windows, matcha Chrome su Windows.
5. **Behavioral analysis (mouse, scroll, keystrokes)**: Obscura è headless, non genera movimenti umani. Per scraping normale non serve.
6. **CDP detection**: Se usiamo CDP, le connessioni WebSocket sono rilevabili. Per `obscura fetch`/`scrape` non usiamo CDP, quindi nessun leak.

### Conclusione

**Sì, possiamo replicare il fingerprint di Chrome al ~99%.** L'unico 1% sono i segnali comportamentali (mouse, scroll) che non servono per scraping automation. Tutte le API JS, headers HTTP, TLS fingerprint, HTTP/2 fingerprint, WebGL, canvas, audio — tutto replicabile.

La differenza tra "approssimare" e "replicare letteralmente" è nei dettagli:
- Property descriptor corretti (non basta il valore)
- `toString()` nativo (non basta definire una funzione)
- Cross-context consistency (worker, iframe)
- Cross-realm identity (non basta un oggetto, serve reference identity)
- Bidirectional references in PluginArray
- Error stack format corretto
- Timing properties
