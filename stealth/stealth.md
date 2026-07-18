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

Extra per TLS/headers perfetti

curl-impersonate-cli → per richieste HTTP pure con fingerprint Chrome byte-exact (utile per fetch senza browser completo)
wafrift-fingerprint o spider_fingerprint → per generare header + script JS da iniettare

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

---

## 21. Gamepad API (`navigator.getGamepads()`)

### Property descriptor

`navigator.getGamepads()` esiste su `Navigator.prototype`:
```
{ writable: true, enumerable: false, configurable: true }
```

### `getGamepads()`

- **`toString()`:** `"function getGamepads() { [native code] }"`
- **Return:** `Array` (non `GamepadList`). Lunghezza fissa **4** in Chrome (indici 0-3). Elementi sono `Gamepad` o `null`.
- **Nessun gamepad connesso:** `[null, null, null, null]`
- **Richiede user gesture** — prima di un button press, restituisce tutti `null`

### `Gamepad` interface (NON esiste `Gamepad` constructor)

Tutte le proprietà sono accessor su `Gamepad.prototype`: `{ get: [native code], set: undefined, enumerable: true, configurable: true }`

| Property | Type | Chrome-specific |
|---|---|---|
| `id` | string | formato: `"<name> (STANDARD GAMEPAD Vendor: <vid> Product: <pid>)"` |
| `index` | long | 0-3 |
| `connected` | boolean | `false` dopo disconnessione |
| `timestamp` | DOMHighResTimeStamp | last update in ms |
| `mapping` | GamepadMappingType | `"standard"` o `""` |
| `axes` | FrozenArray\<double\> | standard = 4 |
| `buttons` | FrozenArray\<GamepadButton\> | standard = 17 |
| `vibrationActuator` | GamepadHapticActuator? | `null` se no hardware |
| `hapticActuators` | FrozenArray | Chrome extension |

### `GamepadButton`

Tutte enumerabili: `pressed` (boolean), `touched` (boolean), `value` (double 0-1).

### `GamepadEvent`

Constructor esposto: `new GamepadEvent(type, { gamepad })`. Event handlers su `Window`: `ongamepadconnected`, `ongamepaddisconnected`.

---

## 22. Credentials Container API (`navigator.credentials`)

### `navigator.credentials` — property descriptor

Su `Navigator.prototype`:
```
{ get: function credentials() { [native code] }, set: undefined, enumerable: true, configurable: true }
```

### `CredentialsContainer` methods

Tutte su `CredentialsContainer.prototype`: `{ writable: true, enumerable: true, configurable: true }`, `toString()` → `[native code]`

| Method | Returns | Parameters |
|---|---|---|
| `get()` | `Promise<Credential\|null>` | `CredentialRequestOptions` |
| `store()` | `Promise<Credential>` | `credential: Credential` |
| `create()` | `Promise<Credential\|null>` | `CredentialCreationOptions` |
| `preventSilentAccess()` | `Promise<undefined>` | — |

### Credential types

| Constructor | Exposed? | Notes |
|---|---|---|
| `PasswordCredential` | **Yes** | `new PasswordCredential(data)` |
| `FederatedCredential` | **Yes** | `new FederatedCredential(data)` |
| `IdentityCredential` | **No** | creato via `get({ identity })` |
| `PublicKeyCredential` | **Yes** | WebAuthn |

### FedCM (Chrome 136+)

- `navigator.credentials.get({ identity: { providers: [{ configURL, clientId }] } })` → `IdentityCredential`
- Multi-IdP: `providers` è array
- Auto-reauthn: max ogni 10 minuti
- `preventSilentAccess()` disabilita auto-reauthn

---

## 23. Origin Private File System (`navigator.storage.getDirectory()`)

### Property descriptor

Su `StorageManager.prototype`:
```
{ value: function getDirectory() { [native code] }, writable: true, enumerable: true, configurable: true }
```

### `StorageManager.getDirectory()`

- Returns `Promise<FileSystemDirectoryHandle>` (name=`""`, kind=`"directory"`)
- **Sempre disponibile** in secure context (HTTPS/localhost)
- **Nessun permesso** richiesto — `queryPermission()` → `"granted"`
- Funziona anche in Incognito (storage effimero)

### `FileSystemDirectoryHandle`

Su `FileSystemDirectoryHandle.prototype`:
`{ writable: true, enumerable: true, configurable: true }` per tutti i metodi.

| Method | Returns |
|---|---|
| `getFileHandle(name, { create })` | `Promise<FileSystemFileHandle>` |
| `getDirectoryHandle(name, { create })` | `Promise<FileSystemDirectoryHandle>` |
| `removeEntry(name, { recursive })` | `Promise<undefined>` |
| `resolve(descendant)` | `Promise<[string, ...]\|null>` |

Proprietà ereditate (readonly accessors): `name`, `kind`, `queryPermission`, `requestPermission`, `isSameEntry`.

### `FileSystemFileHandle`

| Method | Returns | Scope |
|---|---|---|
| `getFile()` | `Promise<File>` | Window + Worker |
| `createWritable({ mode })` | `Promise<FileSystemWritableFileStream>` | Window + Worker |
| `createSyncAccessHandle({ mode })` | `Promise<FileSystemSyncAccessHandle>` | **DedicatedWorker only** |

### `FileSystemSyncAccessHandle` (worker-only)

Metodi sincroni: `read(buffer, {at})`, `write(buffer, {at})`, `truncate(size)`, `getSize()`, `flush()`, `close()`.

### Storage Buckets (`navigator.storageBuckets`)

**Non** `navigator.storage.buckets()`. È una proprietà separata:

```
navigator.storageBuckets → StorageBucketManager
  ├─ open(name, { persisted, durability }) → Promise<StorageBucket>
  │    ├─ name, persist(), persisted(), estimate()
  │    ├─ setExpires(), expires()
  │    ├─ getDirectory() → per-bucket OPFS root
  │    ├─ indexedDB (scoped)
  │    └─ caches (scoped)
  ├─ keys() → Promise<[string, ...]>
  └─ delete(name) → Promise<undefined>
```

Chrome 122+. Secure context required.

---

## 24. Keyboard API (`navigator.keyboard`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: function get keyboard() { [native code] }, set: undefined, enumerable: true, configurable: true }
```

### Methods su `Keyboard.prototype`

| Method | Descriptor | `toString()` |
|---|---|---|
| `getLayoutMap()` | `{ writable: true, enumerable: true, configurable: true }` | `function getLayoutMap() { [native code] }` |
| `lock(keyCodes?)` | `{ writable: true, enumerable: true, configurable: true }` | `function lock() { [native code] }` |
| `unlock()` | `{ writable: true, enumerable: true, configurable: true }` | `function unlock() { [native code] }` |

### `getLayoutMap()` → `KeyboardLayoutMap`

Read-only maplike: `get(key)`, `has(key)`, `size`, `entries()`, `keys()`, `values()`, `forEach()`.
Chrome-specific: i valori dipendono dal layout OS (QWERTY, AZERTY, QWERTZ, Dvorak...).

### `lock()` specifiche

- Richiede fullscreen JS-initiated + transient activation + secure context
- Nessun argomento = locka tutti i tasti
- Escape: long-press Escape per 2 secondi
- Chrome 130-131: provato permission prompt, **ritirato**

---

## 25. Scheduling API (`navigator.scheduling`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Tipo: `Scheduling` object. Chrome M87+.

### `scheduling.isInputPending()`

```
{ writable: true, enumerable: false, configurable: true }
toString() → "function isInputPending() { [native code] }"
```

- **Chrome-only** API — presenza identifica Chrome
- `isInputPending({ includeContinuous: false })` → boolean
- Discrete events: click, keydown, keyup, mousedown, mouseup, touchstart...
- Continuous (solo con `includeContinuous: true`): mousemove, wheel, touchmove...

### `scheduler.currentTaskSignal` (Chrome 136+)

Su `Scheduler.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```

- Returns `TaskSignal | null` (il signal del task corrente via `scheduler.postTask()`)

### `TaskSignal` e `TaskController`

Chrome M94+. `TaskSignal` estende `AbortSignal`. `TaskPriorityChangeEvent` con `previousPriority`.

---

## 26. Web Locks API (`navigator.locks`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Tipo: `LockManager`. Secure context required. **Disponibile anche in Worker.**

### `LockManager` methods

Su `LockManager.prototype`: `{ writable: true, enumerable: true, configurable: true }`, `toString()` → `[native code]`

| Method | Returns |
|---|---|
| `request(name, callback)` | `Promise<any>` |
| `request(name, options, callback)` | `Promise<any>` |
| `query()` | `Promise<{ held: LockInfo[], pending: LockInfo[] }>` |

### `LockInfo`: `{ name: string, mode: "exclusive"|"shared", clientId: string }`

### `Lock`: `{ name: string, mode: string }` (entrambi readonly, `{ get: [native code], set: undefined, enumerable: true, configurable: true }`)

---

## 27. Web Serial API (`navigator.serial`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
**Chrome-only** (non in Firefox/Safari). Secure context required. `[SameObject]`.

### `Serial` methods

| Method | Returns | Notes |
|---|---|---|
| `requestPort({ filters })` | `Promise<SerialPort>` | Richiede user gesture |
| `getPorts()` | `Promise<[SerialPort]>` | Solo già autorizzati |
| `onconnect` | EventHandler | type `"connect"` |
| `ondisconnect` | EventHandler | type `"disconnect"` |

### `SerialPort`

| Method/Property | Description |
|---|---|
| `open({ baudRate, ... })` | dataBits (7/8), stopBits (1/2), parity, flowControl |
| `close()` | — |
| `getInfo()` | `{ usbVendorId, usbProductId, bluetoothServiceClassId }` |
| `readable` | `ReadableStream<Uint8Array> | null` |
| `writable` | `WritableStream<Uint8Array> | null` |
| `connected` | boolean, readonly |

---

## 28. WebUSB API (`navigator.usb`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
**Chrome-only** (Chrome 61+). Secure context req. `[SameObject]`.

### `USB` methods

| Method | Descriptor |
|---|---|
| `requestDevice({ filters })` | `{ writable: true, enumerable: true, configurable: true }` — richiede user gesture |
| `getDevices()` | stessa descrizione — `` return solo già autorizzati |
| `onconnect` / `ondisconnect` | EventHandler (USBConnectionEvent) |

### `USBDevice` properties (tutte readonly, `{ get: [native code], enumerable: true, configurable: true }`)

`usbVersionMajor`, `usbVersionMinor`, `usbVersionSubminor`, `deviceClass`, `deviceSubclass`, `deviceProtocol`, `vendorId`, `productId`, `deviceVersionMajor/Minor/Subminor`, `manufacturerName`, `productName`, `serialNumber`, `configuration`, `configurations`, `opened`.

---

## 29. WebHID API (`navigator.hid`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: false, configurable: false }
```
**Chrome-only** (Chrome 89+). Secure context req. `[SameObject]`.

### `HID` methods

| Method | Returns | Notes |
|---|---|---|
| `requestDevice({ filters })` | `Promise<[HIDDevice]>` | Solo Window, user gesture |
| `getDevices()` | `Promise<[HIDDevice]>` | Worker accessibile |
| `onconnect` / `ondisconnect` | EventHandler | `HIDConnectionEvent` |

### `HIDDevice` properties

`opened`, `vendorId`, `productId`, `productName`, `manufacturerName`, `serialNumber`, `collections` (sequence di `HIDCollectionInfo` con `usagePage`, `usage`, `type`, `children`, `inputReports`, `outputReports`, `featureReports`).

---

## 30. Web Bluetooth API (`navigator.bluetooth`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
**Chrome-only** (Chrome 56+). Secure context req. `[SameObject]`. `writable: false`.

### `Bluetooth` methods

| Method | Returns | Notes |
|---|---|---|
| `getAvailability()` | `Promise<boolean>` | Bluetooth adapter presente? |
| `requestDevice({ filters })` | `Promise<BluetoothDevice>` | User gesture req |
| `onavailabilitychanged` | EventHandler | `BluetoothAvailabilityEvent` |
| `referringDevice` | `BluetoothDevice | null` | — |

### `BluetoothDevice` properties

`id` (opaque, non MAC), `name`, `gatt`, `uuids` (FrozenArray), `connected`, `watchingAdvertisements`. Metodi: `watchAdvertisements()`, `forget()`.

---

## 31. Screen Wake Lock API (`navigator.wakeLock`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: function get wakeLock() { [native code] }, set: undefined, enumerable: true, configurable: true }
```
Secure context req. `[SameObject]`.

### `wakeLock.request("screen")`

```
{ writable: true, enumerable: false, configurable: true }
toString() → "function request() { [native code] }"
```

- Returns `Promise<WakeLockSentinel>`
- **NON richiede user gesture** in Chrome (attualmente)
- Reject: `NotAllowedError` se document hidden o policy blocked

### `WakeLockSentinel`

| Property | Descriptor |
|---|---|
| `released` | `{ get: [native code], set: undefined, enumerable: true, configurable: true }` |
| `type` | `{ get: [native code], set: undefined, enumerable: true, configurable: true }` — sempre `"screen"` |
| `release()` | `{ writable: true, enumerable: false, configurable: true }` → `Promise<undefined>` |

---

## 32. Async Clipboard API (`navigator.clipboard`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: false, configurable: true }
```
**Enumerabile: false** (unico tra le API navigator).

### Metodi

Tutti `{ writable: true, enumerable: false, configurable: true }`, `toString()` → `[native code]`.

| Method | Returns | Permesso richiesto |
|---|---|---|
| `read()` | `Promise<ClipboardItem[]>` | `clipboard-read` + user gesture + HTTPS |
| `readText()` | `Promise<string>` | `clipboard-read` + user gesture + HTTPS |
| `write(items)` | `Promise<undefined>` | `clipboard-write` OR user gesture |
| `writeText(text)` | `Promise<undefined>` | `clipboard-write` OR user gesture |

### `ClipboardItem`

Constructor: `new ClipboardItem({ 'text/plain': blob })`. Properties: `types` (readonly), `presentationStyle` (readonly). Method: `getType(mime)`. Static: `ClipboardItem.supports(mime)`.

**Secure context required** — su HTTP `navigator.clipboard === undefined`.

---

## 33. User Activation API (`navigator.userActivation`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
`writable: false` (readonly). `[SameObject]`. Chrome 72+.

### `UserActivation` properties

| Property | Type | Description |
|---|---|---|
| `hasBeenActive` | boolean | Sticky — `true` se mai ricevuta user activation |
| `isActive` | boolean | Transient — `true` se entro ~5s dall'ultimo gesture |

Entrambi: `{ get: [native code], set: undefined, enumerable: true, configurable: true }`.

Chrome transient activation window: **~5 secondi**. Non disponibile in Worker (`[Exposed=Window]`).

---

## 34. VirtualKeyboard API (`navigator.virtualKeyboard`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Secure context req. `[SameObject]`. **Disponibile anche su desktop** (non solo mobile) in Chrome 136+.

### Properties

| Property | Descriptor | Notes |
|---|---|---|
| `boundingRect` | `{ get, set: undefined, enumerable: true, configurable: true }` | DOMRect, 0 quando nascosto |
| `overlaysContent` | `{ get, set, enumerable: true, configurable: true }` | Read/write boolean |
| `ongeometrychange` | `{ get, set, enumerable: true, configurable: true }` | EventHandler |
| `show()` | `{ writable: true, enumerable: true, configurable: true }` | Richiede sticky activation |
| `hide()` | `{ writable: true, enumerable: true, configurable: true }` | Stessa |

---

## 35. Window Controls Overlay API (`navigator.windowControlsOverlay`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
`[SameObject]`. Sempre esposta su desktop Chrome anche fuori PWA.

### `WindowControlsOverlay`

| Property/Method | Descriptor |
|---|---|
| `visible` | `{ get, set: undefined, enumerable: true, configurable: true }` — boolean |
| `getTitlebarAreaRect()` | `{ writable: true, enumerable: true, configurable: true }` → DOMRect |
| `ongeometrychange` | `{ get, set, enumerable: true, configurable: true }` |

- **Fuori PWA:** `visible=false`, `getTitlebarAreaRect()` → `{x:0, y:0, w:0, h:0}`
- **geometrychange** event: `WindowControlsOverlayGeometryChangeEvent` → `titlebarAreaRect` (DOMRect), `visible` (boolean)

---

## 36. Media Session API (`navigator.mediaSession`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Sempre presente in Chrome (senza user gesture/media). Chrome 57+.

### Properties

| Property | Descriptor | Notes |
|---|---|---|
| `metadata` | `{ get, set, enumerable: true, configurable: true }` | `MediaMetadata | null` |
| `playbackState` | `{ get, set, enumerable: true, configurable: true }` | `"none"` / `"paused"` / `"playing"` |
| `setActionHandler()` | `{ writable: true, enumerable: false, configurable: true }` | — |

### `MediaMetadata`

Constructor: `new MediaMetadata({ title, artist, album, artwork })`. Tutte le properties sono read/write: `title`, `artist`, `album`, `artwork`.

### Action types

`"play"`, `"pause"`, `"seekbackward"`, `"seekforward"`, `"previoustrack"`, `"nexttrack"`, `"stop"`, `"seekto"`, `"skipad"`, `"togglecamera"`, `"togglemicrophone"`, `"hangup"`, `"previousslide"`, `"nextslide"`, `"enterpictureinpicture"`.

---

## 37. WebXR Device API (`navigator.xr`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Secure context req. Chrome 79+. **Chrome-only su desktop** (assente in Firefox/Safari desktop).

### `XRSystem` methods

| Method | Returns | Notes |
|---|---|---|
| `isSessionSupported(mode)` | `Promise<boolean>` | `"inline"`, `"immersive-vr"`, `"immersive-ar"` |
| `requestSession(mode, options?)` | `Promise<XRSession>` | Richiede user gesture |
| `ondevicechange` | EventHandler | — |

### Desktop behavior (senza VR headset)

- `isSessionSupported("inline")` → **true**
- `isSessionSupported("immersive-vr")` → **false** (richiede hardware)
- `requestSession("inline")` → funziona su qualsiasi desktop Chrome
- Feature policy: `xr-spatial-tracking`

---

## 38. MediaCapabilities.encodingInfo()

### Property descriptor

Su `MediaCapabilities.prototype`:
```
{ writable: true, enumerable: false, configurable: true }
toString() → "function encodingInfo() { [native code] }"
```

### Configuration

`type`: `"record"` | `"webrtc"` | `"transmission"`. Campo video/audio con `contentType`, `width`, `height`, `bitrate`, `framerate`.

Returns `Promise<{ supported: boolean, smooth: boolean, powerEfficient: boolean, configuration: object }>`.

### Differenze da `decodingInfo()`

- `type` values diversi (`"record"` vs `"file"`)
- No encrypted media support
- Subset di codec: encoding support ≤ decoding support
- AV1 encoding: solo Profile 0 8-bit è diffuso
- H.264 Baseline encoding: ~99.7%

Tutti i campi di `MediaCapabilitiesInfo`: `{ value: bool, writable: true, enumerable: true, configurable: true }`.

---

## 39. Ink API (`navigator.ink`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
**Chrome-only** (Chrome 94+). Secure context req. `[SameObject]`.

### `Ink` methods

`requestPresenter({ presentationArea })` → `Promise<DelegatedInkTrailPresenter>`
```
{ writable: true, enumerable: false, configurable: true }
toString() → "function requestPresenter() { [native code] }"
```

### `DelegatedInkTrailPresenter`

| Property | Descriptor |
|---|---|
| `presentationArea` | `{ get, set: undefined, enumerable: true, configurable: true }` |
| `updateInkTrailStartPoint(event, { color, diameter })` | `{ writable: true, enumerable: false, configurable: true }` |

- `expectedImprovement` **rimosso** in Chrome 130+
- Disponibile anche su desktop (non solo pen devices)

---

## 40. Navigator Lesser-Known APIs

### `navigator.login` (FedCM Login Status)

```
{ get: [native code], set: undefined, enumerable: false, configurable: true }
```

- **Unico metodo**: `setStatus("logged-in"|"logged-out")` → `undefined`
- FedCM identity requests vanno via `navigator.credentials.get({ identity })`
- `request()`, `cancel()`, `logout()`: rimossi da `navigator.login`

### `navigator.managed` (Managed Configuration API)

```
{ get: [native code], set: undefined, enumerable: false, configurable: true }
```
Chrome-only. `getManagedConfiguration([keys])` → `Promise<object>`. Solo su device enterprise-managed.

### `navigator.devicePosture`

```
{ get: [native code], set: undefined, enumerable: false, configurable: true }
```
Experimental (flag). `type`: `"continuous"` | `"folded"`. `onchange` event.

### `navigator.registerProtocolHandler(scheme, url)`

```
{ writable: true, enumerable: false, configurable: true }
toString() → "function registerProtocolHandler() { [native code] }"
```
Allowed schemes: `mailto`, `bitcoin`, `magnet`, `tel`, `web+*`, ecc. Handler URL must be HTTPS + same-origin.

### `navigator.sendBeacon(url, data)`

```
{ writable: true, enumerable: false, configurable: true }
toString() → "function sendBeacon() { [native code] }"
```
Returns `boolean`. Payload limit: 64 KiB. Sempre HTTP POST.

---

## 41. Trusted Types API & Security APIs

### `window.trustedTypes` (TrustedTypePolicyFactory)

```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Chrome 83+. Cross-origin: `{ value: undefined, writable: false, enumerable: false, configurable: true }`.

| Method | Returns |
|---|---|
| `createPolicy(name, options)` | `TrustedTypePolicy` |
| `isHTML(value)` | `boolean` |
| `isScript(value)` | `boolean` |
| `isScriptURL(value)` | `boolean` |
| `getAttributeType(tag, attr)` | `string\|null` |
| `getPropertyType(tag, prop)` | `string\|null` |

Properties: `emptyHTML` (TrustedHTML), `emptyScript` (TrustedScript), `defaultPolicy` (TrustedTypePolicy | null).

### `TrustedHTML`, `TrustedScript`, `TrustedScriptURL`

**Nessun constructor.** Solo `toString()` (stringifier) e `toJSON()`. Costruttori esposti su `window`: `window.TrustedHTML`, ecc.

### `window.crossOriginIsolated`

```
{ writable: true, enumerable: true, configurable: true }
```
Chrome 87+. `true` se COOP+COEP attivi.

### `window.isSecureContext`

```
{ writable: true, enumerable: true, configurable: true }
```
Cross-origin: `{ value: undefined, writable: false, enumerable: false, configurable: true }`.

### `document.featurePolicy` / `document.permissionsPolicy`

Entrambi `{ get: [native code], set: undefined, enumerable: true, configurable: true }`.

Methods: `allowsFeature(feature)`, `allowsFeature(feature, origin)`, `allowedFeatures()`, `features()`, `getAllowlistForFeature(feature)`.
**Fingerprint:** `features()` restituisce la lista completa delle policy-controlled features supportate — cambia per versione Chrome.

---

## 42. Advanced Performance APIs

### `performance.measureUserAgentSpecificMemory()`

```
{ writable: false, enumerable: true, configurable: true }
toString() → "function measureUserAgentSpecificMemory() { [native code] }"
```
Chrome 89+. Richiede `crossOriginIsolated === true`.

Returns `Promise<MemoryMeasurement>`: `{ bytes, breakdown: [{ bytes, types: [], attribution: [{ url, scope, container }] }] }`.

### `PerformanceObserver.supportedEntryTypes`

Chrome 136+ (main thread): `["element", "event", "first-input", "largest-contentful-paint", "layout-shift", "long-animation-frame", "longtask", "mark", "measure", "navigation", "paint", "resource", "visibility-state"]`. Worker: solo `["mark", "measure", "resource"]`.

### `PerformanceResourceTiming` (Chrome 136+)

Additional properties rispetto a spec standard:
- `deliveryType`: `""` (network) | `"cache"` | `"navigational-prefetch"`
- `contentType`: MIME (es. `"text/javascript"`) o `""` se opaco
- `contentEncoding`: `"gzip"`, `"br"`, ecc.
- `responseStatus`: HTTP status code (0 per opaco)
- `renderBlockingStatus`: `"blocking"` | `"non-blocking"`

Tutte le proprietà sono `{ get: [native code], set: undefined, enumerable: true, configurable: true }`.

### `PerformanceEventTiming` (first-input)

`processingStart`, `processingEnd`, `cancelable`, `target`, `interactionId`, `targetSelector`.

### `LayoutShift`

`value` (0-1), `hadRecentInput`, `lastInputTime`, `sources` (FrozenArray di `LayoutShiftAttribution` con `node`, `previousRect`, `currentRect`).

### `LargestContentfulPaint`

`renderTime`, `loadTime`, `size`, `id`, `url`, `element`.

### `TaskAttributionTiming`

`containerType` (`"iframe"`|`"embed"`|`"object"`|`"window"`), `containerSrc`, `containerId`, `containerName`.

### `VisibilityStateEntry` (Chrome-specific)

`name`: `"visible"` | `"hidden"`, `startTime`: timestamp della transizione.

---

## 43. Service Worker API (`navigator.serviceWorker`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Tipo: `ServiceWorkerContainer`. Solo HTTPS. `[SameObject]`.

### `ServiceWorkerContainer` properties

| Property | Descriptor | Notes |
|---|---|---|
| `controller` | `{ get, set: undefined, enumerable: true, configurable: true }` | `ServiceWorker | null` |
| `ready` | `{ get, set: undefined, enumerable: true, configurable: true }` | `Promise<ServiceWorkerRegistration>` (sempre risolta) |

### Methods

Tutte `{ writable: true, enumerable: true, configurable: true }`, `toString()` → `[native code]`:

| Method | Returns |
|---|---|
| `register(scriptURL, options?)` | `Promise<ServiceWorkerRegistration>` |
| `getRegistration(clientURL?)` | `Promise<ServiceWorkerRegistration | undefined>` |
| `getRegistrations()` | `Promise<sequence<ServiceWorkerRegistration>>` |
| `startMessages()` | `undefined` |

### Event handlers

`oncontrollerchange`, `onmessage`, `onmessageerror`.

### `ServiceWorkerRegistration`

| Property | Type |
|---|---|
| `scope` | `USVString` |
| `active` | `ServiceWorker | null` |
| `installing` | `ServiceWorker | null` |
| `waiting` | `ServiceWorker | null` |
| `navigationPreload` | `NavigationPreloadManager` |
| `pushManager` | `PushManager` |
| `sync` | `SyncManager` |
| `periodicSync` | `PeriodicSyncManager` |
| `updateViaCache` | `"imports" | "all" | "none"` |

---

## 44. Geolocation API (`navigator.geolocation`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Tipo: `Geolocation`. `[SameObject]`.

### Methods

Tutte `{ writable: true, enumerable: true, configurable: true }`, `toString()` → `[native code]`:

| Method | Returns |
|---|---|
| `getCurrentPosition(success, error?, options?)` | `undefined` (async, callback-based) |
| `watchPosition(success, error?, options?)` | `long` (watch ID) |
| `clearWatch(id)` | `undefined` |

### Options dictionary

```
{ enableHighAccuracy: false, timeout: Infinity, maximumAge: 0 }
```

### Senza permesso

- `getCurrentPosition()` chiama `error` callback con `PositionError.code === 1` (PERMISSION_DENIED)
- `Permissions.query({ name: 'geolocation' })` → `{ state: 'denied' | 'prompt' | 'granted' }`

---

## 45. MediaDevices API (`navigator.mediaDevices`)

### Property descriptor

Su `Navigator.prototype`:
```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```
Tipo: `MediaDevices`. `[SameObject]`. Secure context req.

### Methods

Tutte `{ writable: true, enumerable: true, configurable: true }`, `toString()` → `[native code]`:

| Method | Returns | Notes |
|---|---|---|
| `enumerateDevices()` | `Promise<sequence<MediaDeviceInfo>>` | Senza permesso: label/deviceId vuoti |
| `getUserMedia(constraints)` | `Promise<MediaStream>` | Richiede permesso + user gesture |
| `getDisplayMedia(constraints)` | `Promise<MediaStream>` | Richiede permesso + user gesture (screen capture) |
| `getSupportedConstraints()` | `MediaTrackSupportedConstraints` | Chrome-specific set |

### `MediaDeviceInfo` (senza permesso)

```javascript
[
  { deviceId: "", kind: "audioinput", label: "", groupId: "" },
  { deviceId: "", kind: "audiooutput", label: "", groupId: "" },
  { deviceId: "", kind: "videoinput", label: "", groupId: "" }
]
```

### Event handlers

`ondevicechange` su `MediaDevices`.

---

## 46. StorageManager.persist() / persisted()

### Property descriptors

Su `StorageManager.prototype`:
```
{ writable: true, enumerable: true, configurable: true }
toString() → [native code]
```

| Method | Returns |
|---|---|
| `persist()` | `Promise<boolean>` — true se storage diventa persisted |
| `persisted()` | `Promise<boolean>` — true se storage è già persisted |

### Chrome behavior

- `persisted()` → default `false` per la maggior parte delle origini
- `persist()` → Chrome può concedere `true` automaticamente se `"site engages with user"` o l'origine ha `"high site engagement score"`
- **Incognito:** `persisted()` → `false` sempre, `persist()` → `false`
- **Bookmarked/manifest PWA:** più probabile `persist()` → `true`

---

## 47. Navigator misc methods

### `navigator.javaEnabled()`

```
{ writable: true, enumerable: true, configurable: true }
toString() → "function javaEnabled() { [native code] }"
```

- **Sempre restituisce `false`** in Chrome 136+ (Java plugin non supportato da Chrome 45+)
- Ma il metodo **esiste ancora** su `Navigator.prototype`
- Firefox: restituisce `false` (Java plugin rimane in Firefox)
- Safari: non esiste (`undefined`)
- La sua presenza (oltre al valore) è un segnale fingerprint per Chrome/Firefox vs Safari

### `navigator.getBattery()`

```
{ writable: true, enumerable: true, configurable: true }
toString() → "function getBattery() { [native code] }"
```

- Returns `Promise<BatteryManager>`
- **Deprecato** in Chrome 136+ ma **ancora presente**
- `BatteryManager` properties: `charging`, `chargingTime` (0 se charging), `dischargingTime` (Infinity se discharging), `level` (0-1)
- Eventi: `onchargingchange`, `onchargingtimechange`, `ondischargingtimechange`, `onlevelchange`
- Chrome **riduce precisione**: `chargingTime` arrotondato, `level` arrotondato a multipli di 0.01
- In Incognito: `level: 1.0`, `charging: true`, `chargingTime: 0`, `dischargingTime: Infinity`
- `BatteryManager` extende `EventTarget`. Tutte le properties sono `{ get, set: undefined, enumerable: true, configurable: true }`.

### `navigator.share()`

```
{ writable: true, enumerable: true, configurable: true }
toString() → "function share() { [native code] }"
```

- Returns `Promise<undefined>`. Richiede user gesture (transient activation).
- `navigator.share({ title, text, url })` → apre native share sheet
- **Desktop Chrome** 136+: disponibile, ma solo se installato come app/launch handler o tramite flag

### `navigator.canShare(data)`

```
{ writable: true, enumerable: true, configurable: true }
toString() → "function canShare() { [native code] }"
```

- Returns `boolean`. Verifica se `share()` probabilmente funzionerà.
- Non richiede user gesture.
- Chrome-specific: `canShare({ files })` verifica file type support.

---

## 48. Document.cookie property descriptor

### Property descriptor su `Document.prototype`

```
{ get: [native code], set: [native code], enumerable: true, configurable: true }
```

- **Getter e setter entrambi nativi** (non data property)
- `Object.getOwnPropertyDescriptor(Document.prototype, 'cookie')` → accessor descriptor con get e set
- In **cross-origin iframe**: `document.cookie` è accessor che restituisce stringa vuota (no errore)
- In **HTTPS con `SameSite=None`**: il getter restituisce tutti i cookie del dominio
- In **http://**: i cookie con `SameSite=None; Secure` sono invisibili

### Chrome-specific behavior

- `document.cookie = "name=value"` crea cookie session senza `SameSite` specificato → Chrome 136+ lo tratta come `SameSite=Lax`
- Non è possibile leggere cookie `HttpOnly` via JS (il getter li esclude)
- `Object.getOwnPropertyDescriptor(Document.prototype, 'cookie').get.toString()` → `"function get cookie() { [native code] }"`
- `Object.getOwnPropertyDescriptor(Document.prototype, 'cookie').set.toString()` → `"function set cookie() { [native code] }"`

---

## 49. Performance misc properties

### `performance.timeOrigin`

```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```

- Returns `DOMHighResTimeStamp` — epoch ms del navigation start
- Chrome 136+: alta precisione (non arrotondata)
- Costante per tutta la vita della pagina
- `performance.timeOrigin + performance.now()` ≈ `Date.now()` (salvo skew orologio)

### `performance.supportedEntryTypes` (static, alias PerformanceObserver)

Già coperto in §42 ma il **property descriptor statico** è:
```
Object.getOwnPropertyDescriptor(PerformanceObserver, 'supportedEntryTypes')
→ { get: [native code], set: undefined, enumerable: true, configurable: true }
```

### `performance.toJSON()`

```
{ writable: true, enumerable: true, configurable: true }
```
Returns `{ timeOrigin, timing, navigation, memory }`.

---

## 50. Window misc fingerprint surfaces

### `window.originAgentCluster`

```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```

- Chrome 88+. Tipo: `boolean`.
- `true` se l'agente è origin-keyed (con `Origin-Agent-Cluster: ?1` header)
- `false` di default

### `window.credentialless`

```
{ get: [native code], set: undefined, enumerable: true, configurable: true }
```

- Chrome 117+. Tipo: `boolean`.
- `true` se l'iframe è stato creato con `credentialless` attribute
- Solo su iframe, non su top-level window

### `window.opener`

- Property descriptor: `{ get: [native code], set: [native code], enumerable: true, configurable: true }` (accessor con get e set)
- In popup aperto via `window.open()`: riferimento alla window padre
- Con `noopener`: `null`

### `window.locationbar`, `window.menubar`, `window.personalbar`, `window.scrollbars`, `window.statusbar`, `window.toolbar`

Tutte `BarProp` objects su `Window.prototype`.

```
Object.getOwnPropertyDescriptor(Window.prototype, 'locationbar')
→ { get: [native code], set: undefined, enumerable: true, configurable: true }
```

Ogni `BarProp` ha una sola property: `visible` → boolean
```
BarProp.prototype.visible → { get: [native code], set: undefined, enumerable: true, configurable: true }
```

In headless Chrome 136+:
- `locationbar.visible` → `false` (URL bar non visibile)
- `menubar.visible` → `false`
- `personalbar.visible` → `false`
- `scrollbars.visible` → `true` (sempre true anche in headless)
- `statusbar.visible` → `false`
- `toolbar.visible` → `false`

---

## 51. Navigator prototype — checklist finale

Tutte le proprietà su `Navigator.prototype` in Chrome 136+ (lista completa, aggiornata):

```
bluetooth, canShare, clipboard, connection, cookieEnabled,
credentials, deviceMemory, doNotTrack (getter deprecated),
getBattery, getGamepads, geolocation, gpu, hardwareConcurrency,
hid, ink, javaEnabled, keyboard, language, languages, locks,
login, managed, maxTouchPoints, mediaCapabilities, mediaDevices,
mediaSession, mimeTypes, onLine, pdfViewerEnabled, permissions,
platform, plugins, product, registerProtocolHandler, scheduling,
sendBeacon, serial, serviceWorker, share, storage, storageBuckets,
userActivation, userAgent, userAgentData, usb, vendor,
virtualKeyboard, wakeLock, webdriver, windowControlsOverlay, xr
```

**Assenti in Chrome (presenti in altri browser):**
```
buildID (Firefox), oscpu (Firefox), globalPrivacyControl (Firefox/Brave),
standalone (Safari iOS), contacts (Safari), securitypolicy (Firefox)
```

**Deprecati/rimossi:**
```
vendorSub (Chrome 136+: ancora presente ma vuoto), productSub (ancora presente),
taintEnabled (rimosso da Chrome), preference (rimosso)
```

---

## 52. Window.prototype — property descriptors checklist

Tutte le proprietà fingerprint-critical su `Window.prototype` in Chrome 136+:

| Property | Descriptor type | Notes |
|---|---|---|
| `onblur, onfocus, onresize, onscroll, onload, onunload` | `{ get, set }` accessor | EventHandler IDL |
| `onbeforeunload, onhashchange, onpopstate` | `{ get, set }` accessor | EventHandler IDL |
| `ondevicemotion, ondeviceorientation` | `{ get, set }` accessor | Solo mobile |
| `onlanguagechange` | `{ get, set }` accessor | EventHandler |
| `onrejectionhandled, onunhandledrejection` | `{ get, set }` accessor | Promise events |
| `onsubmit, onreset` | `{ get, set }` accessor | Form events |
| `ontouchstart, ontouchmove, ontouchend, ontouchcancel` | `{ get, set }` accessor | Chrome ha **sempre** questi anche su desktop (diverso da Safari/Firefox) |
| `ongotpointercapture, onlostpointercapture` | `{ get, set }` accessor | — |
| `onsecuritypolicyviolation` | `{ get, set }` accessor | CSP violations |
| `print()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `alert(), confirm(), prompt()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `open()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `postMessage()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `requestAnimationFrame()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `cancelAnimationFrame()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `requestIdleCallback()` | `{ writable: true, enumerable: true, configurable: true }` | Chrome-only (non Safari) |
| `cancelIdleCallback()` | `{ writable: true, enumerable: true, configurable: true }` | Chrome-only |
| `queueMicrotask()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `structuredClone()` | `{ writable: true, enumerable: true, configurable: true }` | Chrome 98+ |
| `createImageBitmap()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `fetch()` | `{ writable: true, enumerable: true, configurable: true }` | — |
| `reportError()` | `{ writable: true, enumerable: true, configurable: true }` | Chrome 95+ |
| `btoa(), atob()` | `{ writable: true, enumerable: true, configurable: true }` | — |

Tutti i getter `toString()` → `"function get <name>() { [native code] }"`.
Tutti i setter `toString()` → `"function set <name>() { [native code] }"`.
Tutte le funzioni `toString()` → `"function <name>() { [native code] }"`.
