# Browser Fingerprinting & Bot Detection Research

> Comprehensive technical findings on browser fingerprinting techniques,
> bot detection signals, and anti-authentication measures as of mid-2026.
>
> Sources: FingerprintJS, CreepJS, BotD, Cloudflare, Akamai, DataDome,
> PerimeterX (HUMAN Security), reCAPTCHA v3, hCaptcha, and 20+ vendor
> reverse-engineering studies.

---

## Table of Contents

1.  [FingerprintJS — Headless Chrome Detection](#1-fingerprintjs--headless-chrome-detection)
2.  [CreepJS — Lie Detection & Headless Rating](#2-creepjs--lie-detection--headless-rating)
3.  [BotD — FingerprintJS's Bot Detection Library](#3-botd--fingerprintjss-bot-detection-library)
4.  [Cloudflare — Bot Detection & JS Challenges](#4-cloudflare--bot-detection--js-challenges)
5.  [Akamai — Bot Manager & Sensor Data](#5-akamai--bot-manager--amp-sensor-data)
6.  [DataDome — TLS-First Detection & Picasso Challenge](#6-datadome--tls-first-detection--picasso-challenge)
7.  [PerimeterX (HUMAN Security) — Behavioral Biometrics](#7-perimeterx-human-security--behavioral-biometrics)
8.  [reCAPTCHA v3 & hCaptcha — Scoring & Fingerprinting](#8-recaptcha-v3--hcaptcha--scoring--fingerprinting)
9.  [Complete Detection Vectors — Exhaustive Checklist](#9-complete-detection-vectors--exhaustive-checklist)

---

## 1. FingerprintJS — Headless Chrome Detection

### Overview

FingerprintJS ships two products with fundamentally different architectures:

- **Open-source** (`@fingerprintjs/fingerprintjs`): MIT-licensed, runs
  entirely client-side. Current line is v5 (v5.2.0 as of April 2026).
  Fingerprints are computed and compared in-browser. The README admits
  accuracy is "significantly lower" than the commercial version.
- **Fingerprint Pro**: The JavaScript agent collects ~100 browser and
  device signals and ships them to Fingerprint's backend. The backend
  returns a stable identifier plus **Smart Signals**. Accuracy: ~99.5%.

### Open-Source Signal Sources (v5 tree)

As of the v5.x source tree, the registered entropy sources are:

- `userAgent` / `userAgentData` (Client Hints)
- `webGlBasics` — renderer, vendor, version strings
- `webGlExtensions` — supported WebGL extensions list
- `canvas` — text + shape drawing pixel hash
- `audio` — oscillator through Web Audio graph, floating-point output
- `fonts` — installed font enumeration via font measuring
- `plugins` — plugin array (deprecated, frozen in modern Chrome)
- `screenFrame` / `screenResolution` / `devicePixelRatio`
- `colorDepth` / `pixelDepth`
- `hardwareConcurrency` / `deviceMemory`
- `timezone` / `timezoneOffset`
- `languages` / `locale` (`Intl.DateTimeFormat`)
- `sessionStorage` / `localStorage` / `indexedDB` availability
- `cpuClass` / `platform` / `vendor` / `vendorFlavors`
- `colorGamut` / `contrast` / `reducedMotion` / `hdr` (media queries)
- `videoCard` / `audioBaseLatency`
- `architecture` / `applePay` / `privateClickMeasurement`
- `touchSupport` / `osCpu` / `cookiesEnabled`
- `domBlockers` — injects bait elements with ad-blocker class names,
  checks which get hidden
- `fonts` — measures rendered text width for known fonts
- `math` — trigonometric precision fingerprint
- `indexedDB` — quota/availability checks

### Fingerprint Pro Smart Signals

Server-side derived signals (JSON field names):

| Field | Scope | Description |
|-------|-------|-------------|
| `suspect_score` | Common | Weighted integer combining all other signals |
| `velocity` | Common | Distinct IPs/countries/linked_ids across 5m/1h/24h windows |
| `ip_info` / `ip_blocklist` | Common | IP reputation data |
| `proxy` / `proxy_confidence` | Common | Proxy/VPN detection |
| `bot` / `bot_type` | Browser | Bot detection result + type |
| `incognito` | Browser | Private mode detection |
| `vpn` / `vpn_confidence` | Browser | VPN detection with confidence |
| `tampering` / `tampering_confidence` | Browser | Anti-detect browser / spoofing detection |
| `virtual_machine` | Browser | VM detection |
| `privacy_settings` | Browser | Privacy-related browser settings |
| `developer_tools` | Browser | DevTools open detection |

### How FingerprintJS Detects Automation (BotD + Pro)

- **`navigator.webdriver`** — the spec-mandated automation flag
- **`HeadlessChrome` UA token** — stripped by stealth tools but catches
  lazy configurations
- **`window.chrome` object shape** — real Chrome has specific nested
  methods (`csi()`, `loadTimes()`, `runtime`); headless/stealth stubs
  often miss subtleties
- **Plugin array coherence** — headless defaults to empty or fixed array
- **Permission inconsistencies** — `Notification.permission` vs
  `permissions.query()` disagreement
- **WebGL renderer** — SwiftShader / llvmpipe vs real GPU
- **Canvas fingerprint deviation** — headless rendering differs
- **AudioContext** — headless environments often have no audio stack or
  software-only implementation
- **Cross-layer consistency** — Pro checks timezone vs IP geolocation,
  OS vs TLS fingerprint, etc.

### VPN Detection (Pro)

Methods documented in Smart Signals:
- Timezone mismatch (browser JS timezone vs IP geolocation)
- Known public-VPN provider IP ranges
- OS-versus-IP mismatch (browser claims Windows, IP geolocates to
  datacenter)
- Relay-service identification

### Incognito Detection (Pro)

Publicly documented history of techniques (current internal method not
published):
- Storage-quota method (`navigator.storage.estimate()` < ~120 MB) —
  worked Chrome 74–84, now patched
- Filesystem timing method — writes faster against smaller incognito
  quota, still works but unreliable
- Firefox-specific: `indexedDB.open()` throws in private mode
- Safari-specific: `localStorage` write failures (patched Safari 14+)

---

## 2. CreepJS — Lie Detection & Headless Rating

### Overview

CreepJS is an open-source research project that does not just collect
fingerprint signals — it cross-checks them for internal consistency.
The core idea: **"trusted" vs "untrusted" sources** for the same fact.

GitHub: `abrahamjuliot/creepjs`

### CreepJS's Unique Methodology

**1. Prototype Lie Detection (`src/lies/index.ts`)**
- Checks `.toString()` output of native functions (should be
  `function foo() { [native code] }`)
- Checks prototype chain consistency
- Checks whether functions produce `TypeError` under conditions a
  real native implementation would
- Proxy detection: creates three separate Proxy objects around a
  function and tests for recursion errors and prototype cycle behavior
  that genuine functions handle differently than Proxied ones

**2. Cross-Context Consistency Checks**
- Main-thread vs Worker context — the same computation (Canvas draw,
  timezone read) runs in both; spoofing tools frequently patch only
  main thread and miss the Worker's separate global scope
- Direct read vs derived value — reads a property directly and also
  infers it indirectly from side effects, error messages, or behavior
- Declared identity vs observable behavior — UA string claims one
  engine, but JS engine timing and error-message wording reveal the
  real engine

**3. Resistance Detection (`src/resistance/index.ts`)**
- Timer precision clamping: fires 10 delayed timestamps and checks if
  last digit is always the same (Firefox RFP rounds `Date.now()` to
  nearest 100ms)

**4. Headless Detection Module (`src/headless/index.ts`)**
- `ActiveText` renders as `rgb(255, 0, 0)` — known headless default
- Permissions API contradictory states
- Taskbar presence: compares `screen.height` vs `screen.availHeight`
- `document.hasTrustToken()` — headless fails
- `window.chrome` invalid index location relative to stable property
- `Function.prototype.toString` — detects missing `Function.toString`
  line in error stack
- Try/catch `wakeLock` — headless fails

### Signals Collected

CreepJS collects ~20+ independent signal categories:
1. `contentWindow` (Self) object
2. CSS System Styles
3. CSS Computed Styles
4. HTMLElement
5. JS Runtime (Math)
6. JS Engine (Console Errors)
7. Emojis (DomRect)
8. DomRect
9. SVG
10. Audio
11. MimeTypes
12. Canvas (Image, Blob, Paint, Text, Emoji)
13. TextMetrics
14. WebGL
15. GPU Params (WebGL Parameters)
16. GPU Model (WebGL Renderer)
17. Fonts
18. Voices (speechSynthesis)
19. Screen
20. Resistance (Known Patterns)
21. Device of Timezone
22. Media Devices
23. Navigator properties

### Automation-Framework Specific Detection

Detects these globals that automation tools inject:
- `window.cdc_adoQpoasnfa76pfcZLmcfl_Array` (and Promise, Symbol,
  Proxy, Object) — ChromeDriver artifacts
- `window.__nightmare`
- `window.OSMJIF`
- `window._Selenium_IDE_Recorder`
- `window.__$webdriverAsyncExecutor`
- `window.__driver_evaluate` / `__driver_unwrapped`
- `window.__fxdriver_evaluate` / `__fxdriver_unwrapped`
- `window.__lastWatirAlert` / `__lastWatirConfirm` / `__lastWatirPrompt`
- `window.__phantomas`
- `window.__selenium_evaluate` / `__selenium_unwrapped`
- `window.__webdriver__chr`
- `window.__webdriver_evaluate` / `__webdriver_unwrapped`
- `window.__webdriver_script_fn` / `__webdriver_script_func`
- `window.awesomium`
- `window.callSelenium` / `calledPhantom` / `calledSelenium`
- `window.domAutomationController`
- `window.fmget_targets`
- `window.geb`
- `document.$chrome_asyncScriptInfo`

### Trust Score Model

- Uses a Bayesian score combining all signals
- If browser is Chromium and OS isn't Linux, that's flagged as
  suspicious (bot indicator)
- Has `headlessRating`, `stealth` scores
- Produces `botHash` and `fuzzyHash` composites

### Known Bypass Difficulty

- Regular Playwright: 100% headless detection
- Patchright (Playwright patch): 67% headless, 0% stealth
- Undetected-chromedriver: 67% headless
- Puppeteer + stealth plugin: 33% headless, 80% stealth
- Nodriver: 67% headless
- Headful mode + virtual display: 0% headless
- Camoufox (Firefox-based): 0% headless, 0% stealth, 6% "like headless"

---

## 3. BotD — FingerprintJS's Bot Detection Library

### Overview

BotD is a separate MIT-licensed open-source library
(`@fingerprintjs/botd`) that runs entirely client-side. Its job is
narrower than fingerprinting: decide whether the current environment is
an automation tool.

**Status**: In maintenance mode — critical fixes only, new features
unlikely.

### What BotD Detects

- **Headless browsers** (headless Chrome, headless Firefox)
- **Selenium** — via `navigator.webdriver`, ChromeDriver artifacts
- **Puppeteer** — via CDP artifacts, injected script residue
- **Playwright** — via `__playwright__binding__` and CDP artifacts
- **PhantomJS** — via globals like `window.callPhantom`
- **Nightmare** — via `window.__nightmare`
- **Electron** — via `window.process` and `require()` availability
- **Cypress** — via `window.Cypress` and `window.cy`
- **Generic automation** — via property descriptor tampering detection

### Detection Techniques

- `navigator.webdriver === true`
- `HeadlessChrome` in User-Agent string
- `window.chrome` object shape and property presence
- Plugin array length and content
- Permission API inconsistency
- Known automation framework globals
- `Function.prototype.toString` on native functions (checks for
  patched implementations)
- Screen dimension anomalies (800x600 default, no taskbar offset)

### BotD vs Pro Bot Detection

BotD is purely client-side and detects obvious tells. Pro's bot
detection adds:
- Server-side cross-layer consistency (TLS fingerprint vs UA,
  IP reputation, etc.)
- Behavioral analysis (mouse movement patterns, timing distributions)
- ML-based classification across 100+ signals
- Session history and velocity tracking

---

## 4. Cloudflare — Bot Detection & JS Challenges

### Overview

Cloudflare's bot detection is the most widely deployed on the web. It
operates across 6+ independent layers that feed into a ML model
producing a **bot score from 1 (human) to 99 (bot)**.

### Detection Layers

#### Layer 1: TLS Fingerprinting (Pre-HTTP)
- JA3/JA4 hash from TLS ClientHello
- Compared against database of known browser fingerprints
- Cannot be spoofed from JavaScript
- Available in Cloudflare WAF firewall rules

#### Layer 2: HTTP/2 Fingerprinting
- SETTINGS frame values (initial window size, max concurrent streams)
- Pseudo-header order (`:method`, `:authority`, `:scheme`, `:path`)
- Header ordering at HPACK level
- WINDOW_UPDATE timing and values
- Stream priority behavior

#### Layer 3: HTTP Header Analysis
- User-Agent consistency with other `Sec-CH-UA` Client Hints
- Header order (browsers emit headers in deterministic order)
- Missing or inconsistent `Accept-Language`, `Accept-Encoding`
- Coherence between `Sec-CH-UA` brand list and reported UA
- `Sec-Fetch-*` header presence and values

#### Layer 4: JavaScript Detections (JSD)
Cloudflare's **JavaScript Detections** engine runs on every HTML page
request. It injects a lightweight, invisible script that:

- Probes browser environment (navigator properties, canvas, WebGL,
  audio, fonts, screen)
- Checks for headless/automation indicators
- Issues a `cf_clearance` cookie with result:
  `cf.bot_management.js_detection.passed` = `true` / `false`
- Lifespan: 15 minutes, auto-refreshed
- Uses a separate thread where available to minimize perf impact

Signals checked by JSD:
- `navigator.webdriver`
- `navigator.plugins` length and content
- `navigator.languages`
- `navigator.hardwareConcurrency`
- `navigator.deviceMemory`
- `window.chrome` object integrity
- Canvas fingerprint hash
- WebGL renderer string
- AudioContext fingerprint
- Performance API timing patterns
- Property descriptor integrity
- Prototype chain checks

#### Layer 5: Behavioral Analysis — Precursor (New in 2026)
Cloudflare's **Precursor** system (announced July 2026) is a
session-based behavioral verification system:

- Continuously collects interaction data via injected script
- Captures: pointer movement, keyboard activity, focus changes,
  page visibility
- Evaluators cross-reference data (e.g., pointer activity vs page
  visibility duration)
- Session-scoped — cannot reset behavior by refreshing page
- Feeds into bot score in real-time
- Uses Web Workers for non-blocking collection

#### Layer 6: IP Reputation & ML
- Historical data from 1 trillion+ requests/day across 20%+ of the web
- Known datacenter IP ranges, proxy/VPN providers
- Behavioral clustering across sessions
- Rate limiting and velocity tracking
- Machine learning model trained on global traffic patterns

### Challenge Types

| Type | UX | When Fired |
|------|----|-----------|
| **JS Challenge** | "Checking your browser..." page, 2-5s proof-of-work | Suspicious but not conclusive |
| **Managed Challenge** | Adaptive: may be invisible or show Turnstile | Medium-risk scores |
| **Turnstile** | Invisible CAPTCHA replacement, behavioral + PoW | Varies by site config |
| **Interactive Challenge** | Legacy CAPTCHA (deprecated) | High-risk or repeated offenses |

### `cf-mitigated: challenge` Header

When Cloudflare issues a challenge, the response includes:
```
cf-mitigated: challenge
accept-ch: Sec-CH-UA-Bitness, Sec-CH-UA-Arch, ...
critical-ch: ...
```

This header signals that the request was intercepted and a challenge
must be resolved before content is served. The server also sends a
`__cf_bm` cookie to smooth out bot scores across sessions.

### What Cloudflare Checks in JS Challenges

The challenge page runs:
1. **Proof-of-work computation** — takes 1-5 seconds on real hardware,
   slows batching
2. **Browser environment probes** — all JS-level signals
3. **Cryptographic token generation** — time-limited, bound to session
4. **Client Hints validation** — verifies `Accept-CH` response
   (headless browsers often fail to send requested high-entropy hints)

---

## 5. Akamai — Bot Manager & Sensor Data

### Overview

Akamai Bot Manager protects ~30% of the Fortune 500. It scores clients
from **0 (human) to 100 (bot)** using a multi-layer approach.
Detection is split into two phases: network-layer (pre-JS) and
sensor-data (JS-based).

### Phase 1: Network-Layer Detection (Pre-JS)

**TLS Fingerprinting (JA4+)**
- Analyzed at the EdgeWorker before any HTML is sent
- Compares JA3/JA4 against expected browser signatures
- TLS mismatch alone can trigger a block before page load

**HTTP/2 Fingerprinting**
- SETTINGS parameters: initial window size, max concurrent streams
- Pseudo-header ordering
- Header order at HPACK level
- Akamai uses the format `1:65536;2:0;...|WINDOW|PRIORITY|HEADERS`

**HTTP/3 / QUIC**
- Growing share of Akamai-fronted properties advertise h3
- A client that drops to h2 every time stands out

**Cookie Continuity**
- `bm_sz` — set server-side on first HTML response, ~4h lifetime
- Used as a seed for sensor encoding
- You cannot generate a valid sensor for a session using a `bm_sz`
  from a different session

### Phase 2: Sensor Data (`sensor.js` / `_sec/cp_challenge/verify`)

The sensor script is a massive (~512 KB) obfuscated JavaScript file
that collects 100+ signals and POSTs them to Akamai's endpoint.

**Categories and Signals:**

| Category | Signals Count | Examples |
|----------|---------------|---------|
| Browser fingerprint | 20+ | UA, plugins, screen, color depth |
| Hardware | 10+ | Device memory, CPU cores, GPU, touch support |
| Behavioral | 15+ | Mouse paths, click cadence, keystrokes, scroll |
| JS environment | 25+ | Prototype chains, function arity, error messages |
| Timing | 10+ | Navigation Timing, Paint Timing, execution timing |
| Network | 5+ | Connection type, RTT, downlink speed |

**Key Detection Techniques:**

1. **Canvas fingerprint** — renders text and shapes, reads pixels back,
   hashes result
2. **WebGL fingerprint** — GPU vendor/renderer strings via
   `WEBGL_debug_renderer_info`
3. **AudioContext fingerprint** — synthetic audio graph output
4. **Font enumeration** — installed fonts on system
5. **60 extension probe** — fires 60 `chrome-extension://` fetch
   requests to known extension manifest URLs (uBlock, LastPass,
   Bitwarden, etc.). Real users have some installed; headless has
   none, so all 60 fail — a statistically impossible pattern
6. **Prototype poisoning detection** — checks if native prototypes
   have been modified, function arity, `.toString()` output
7. **Coherence block** — cross-checks `navigator.webdriver`,
   `window.chrome` shape, `callPhantom` / `window.opera` globals,
   `mozInnerScreenY`, etc.
8. **Timing traps** — measures execution time to detect debuggers,
   sandboxing, or non-standard environments

### The `_abck` Cookie Lifecycle

1. `_abck` starts at `~-1~` (not trusted)
2. After sensor POST succeeds, flips to `~0~` (trusted)
3. If any signal contradicts the request fingerprint, the cookie is
   immediately invalidated — **single mismatch = bot**
4. Protected endpoints check `_abck` state; returns 412 if still `~-1~`

### Pixel Challenge (`ak_bmsc`)

A lightweight fingerprinting beacon that runs on some pages:
- Collects screen/window geometry, hardware hints, math checks
- Posts to Akamai endpoint
- Returns `ak_bmsc` cookie (~2h lifetime)
- Uses identifier `bazadebezolkohpepadr` in the obfuscated script

### `sec-cpt` Interstitial

A proof-of-work challenge (428 status + countdown timer) that fires
when the score crosses a threshold. Requires:
- `bm_sz` and `_abck` present
- Crypto challenge payload generation
- Typically 5+ second forced wait

### Sensor Obfuscation

- v3: Inline VM execution with LCG-keyed substitution cipher
- v4/v5: VM-in-VM architecture, polynomial-based constant generation,
  cryptographic integrity validation, daily script rotation
- Code changes between deployments — no static bypass possible

---

## 6. DataDome — TLS-First Detection & Picasso Challenge

### Overview

DataDome is architecturally unique among major bot detection vendors.
Detection begins at the **TLS handshake**, before any JavaScript
executes. It uses 5 simultaneous detection layers.

### Layer 1: TLS Fingerprinting (Network Layer)

- JA3/JA4 analysis of TLS ClientHello
- Performed before any HTTP content is served
- Headless Chrome, Puppeteer, Playwright all have distinct JA4
  fingerprints from real browsers
- Non-browser TLS fingerprints blocked immediately

### Layer 2: HTTP/2 & Header Analysis

- HTTP/2 SETTINGS frame fingerprinting
- Pseudo-header order (the 4th field in Akamai's format:
  `:method`, `:scheme`, `:authority`, `:path` order)
- Header order at HPACK level (browsers emit headers in
  version-stable order)
- Client Hints presence and coherence
- TCP-level fingerprinting: initial TTL (Windows ~128, Linux ~64),
  TCP options ordering (JA4T)

### Layer 3: JavaScript Agent

Injected on every page, collects:
- All navigator properties
- Canvas fingerprint (full rendering, not just hash)
- WebGL renderer + extensions
- AudioContext fingerprint
- Screen metrics
- Font enumeration
- Timezone / language
- `navigator.webdriver`, `window.chrome`, plugins
- CDP detection via `Runtime.enable` side effects

### Layer 4: The "Picasso" Challenge (Unique to DataDome)

This is DataDome's primary differentiator — visual rendering validation:

1. Server sends graphical rendering instructions to the client
2. Browser must execute these using Canvas/WebGL
3. Rendered output sent back to DataDome
4. Server verifies output matches what the claimed browser/OS
   combination should produce

**Why it works:**
- Real Chrome on macOS produces a specific pixel-perfect output
- Chrome on Windows produces different output (different font
  rendering engine)
- Headless Chrome in Docker produces yet another output (no GPU,
  software rendering)
- If the "Picasso" output doesn't match the claimed UA + platform,
  the request is flagged

You cannot just *say* you're "Chrome on macOS" — you must *render*
like Chrome on macOS.

### Layer 5: WASM Challenges

DataDome also uses WebAssembly-based challenges:
- Browser must execute a compiled state machine
- Produces a specific output verifiable server-side
- Computationally expensive to solve without actually executing
  the WASM binary in a real browser

### Detection Sequence

```
1st Request → TLS fingerprint check → Header/HTTP2 check
  → JS tag loads → Picasso challenge (if needed)
  → Device Check interstitial (if score < threshold)
  → WASM proof-of-work (optional)
```

### Unique Detection Characteristics

- **IP reputation is aggressive**: datacenter IPs blocked by default
  in many configurations
- **Single-request scoring**: unlike Akamai's accumulated trust,
  DataDome scores each request independently
- **CDP detection was pioneered here**: DataDome published the
  `Runtime.enable` side-effect detection technique
- **Detection model updates**: multiple times per week

---

## 7. PerimeterX (HUMAN Security) — Behavioral Biometrics

### Overview

PerimeterX (acquired by HUMAN Security in 2022) is the most
**behavioral-signal-heavy** major bot detection vendor. It combines
device fingerprinting, TLS fingerprinting, and behavioral biometrics.

Cookies: `_px3`, `_pxhd`

### Detection Signals

#### TLS & Network (First-Line Filter)
- JA3/JA4 fingerprinting — blocks HTTP libraries before sensor loads
- IP reputation — datacenter and proxy IPs flagged
- HTTP/2 fingerprinting

#### Browser Environment (JS Sensor)
- Canvas fingerprint (full rendering + hash)
- WebGL renderer string (flags SwiftShader/Mesa)
- Screen metrics (outer vs inner dimension mismatches)
- Navigator properties (webdriver, plugins, languages,
  hardwareConcurrency, deviceMemory, platform)
- Font enumeration
- AudioContext fingerprint
- CDP event monitoring — checks for automation-related CDP domains
  (`Runtime.evaluate`, `Page.addScriptToEvaluateOnNewDocument`)

#### Behavioral Biometrics (PerimeterX's Core Strength)

| Signal | What It Measures |
|--------|-----------------|
| **Mouse movement** | Trajectory smoothness, acceleration curves, micro-jitter, Bezier characteristics |
| **Click patterns** | Time to first interaction, click duration, position distribution, double-click intervals |
| **Scroll behavior** | Velocity, direction changes, relationship between scroll and content visibility |
| **Touch events** | Pressure, contact area, multi-touch patterns (mobile) |
| **Timing** | Interval between page load and sensor submission (real: 200-800ms; bot: 50ms) |
| **Key press dynamics** | Key-down/key-up intervals, form fill patterns, paste detection |

#### Canvas Fingerprint Cross-Validation
PerimeterX does not just collect a canvas hash — it **compares** the
fingerprint against expected values for the claimed browser + GPU
combination:
- Claiming Chrome on Windows with NVIDIA GPU
- But producing a canvas fingerprint consistent with Linux software
  renderer
- → Mismatch flag, score penalty

### Detection Approach Differences

| Feature | PerimeterX | DataDome | Akamai |
|---------|-----------|----------|--------|
| Primary strength | Behavioral biometrics | IP reputation + TLS | Sensor fingerprint |
| Challenge type | Press & Hold, CAPTCHA | CAPTCHA, 403 | Silent, sec-cpt |
| Behavioral weight | Very high | Medium | High |
| IP reputation | High | Very high | Medium |
| Sensor update frequency | High (dynamic obfuscation) | Medium | Very high (daily+) |

---

## 8. reCAPTCHA v3 & hCaptcha — Scoring & Fingerprinting

### reCAPTCHA v3

#### Scoring Model
- Returns score from **0.0 (bot) to 1.0 (human)**
- Free tier returns only 4 discrete values: 0.1, 0.3, 0.7, 0.9
- Enterprise provides 11-level scale + reason codes
- Tokens expire after 2 minutes, must be fetched at submission point

#### Five Signal Categories

**1. Browser Signals (High weight)**
- `navigator.webdriver` (true → -0.3 to -0.5)
- `navigator.plugins` (empty → penalty)
- `navigator.languages` (missing/empty → penalty)
- `navigator.hardwareConcurrency`, `deviceMemory`
- `window.chrome` object integrity
- `Notification.permission` behavior
- Canvas fingerprint (headless pattern → -0.3 to -0.5)
- WebGL renderer (SwiftShader → -0.3 to -0.5)

**2. Behavioral Signals (Very high weight)**
- Mouse movement count, trajectory, acceleration, micro-movements,
  target overshoot (no mouse movement → -0.4 to -0.7)
- Keyboard inter-key timing (instant paste → 0.1-0.2 drop)
- Scroll patterns (no scroll before form → -0.2 to -0.3)
- Element interaction sequence (direct-to-form → -0.1 to -0.3)

**3. Network Signals (Medium-high weight)**
- IP reputation (datacenter → -0.3 to -0.5, Tor → -0.4 to -0.6)
- TLS fingerprint / JA3 (non-browser → -0.2 to -0.4)
- HTTP header coherence (Client Hints, Accept-Language, etc.)

**4. History Signals (Medium weight)**
- Google cookie age and session history
- `NID` / `__Secure-3PSID` / `__Secure-3PAPISID` presence
- Previous reCAPTCHA success/failure on this IP
- Cross-site abuse patterns

**5. Environmental Signals**
- Timezone consistent with IP geolocation
- Screen dimensions plausible for claimed OS
- Language matches IP country

#### Enterprise Reason Codes
- `AUTOMATION` — behavior matches automated agent
- `UNEXPECTED_ENVIRONMENT` — illegitimate environment for this site
- `TOO_MUCH_TRAFFIC` — abnormal traffic volume
- `UNEXPECTED_USAGE_PATTERNS` — behavior diverges from baseline
- `LOW_CONFIDENCE_SCORE` — insufficient traffic data

#### reCAPTCHA Fingerprint Architecture

Based on reverse-engineering of the reCAPTCHA inner VM:
- Fingerprint signals collected in subfields: `[value, key, elapsed]`
- `elapsed` = time to collect + encrypt value — detects fast execution,
  hooks, breakpoints, sandboxing
- Signal codes aggregated, hashed, encrypted
- Bloom filter fingerprint of all DOM nodes (tag names, attributes,
  text content) — 240-bit, 7 rounds, max 25 nodes
- Behavior checksum: interaction score derived from mouse clicks,
  keyboard, focus events
- Session token (`rc::a`, `rc::b`, `rc::c` cookies) significantly
  boosts score if valid

### hCaptcha

#### Score Model (Enterprise/BotStop)
- Returns score from **0.0 (no risk) to 1.0 (confirmed threat)**
- **Polarity inverted vs reCAPTCHA** — 1.0 in reCAPTCHA = human,
  1.0 in hCaptcha = bot
- Reason codes: `score_reason` array explains risk drivers
- Does NOT publish a complete vocabulary (partially reverse-engineered)

#### Detection Signals
- IP reputation and rate limiting
- Browser environment analysis (similar signal set to reCAPTCHA)
- Mouse movement, key events, touch events
- Page timing and interaction patterns
- Proof-of-work stamp (hashcash-style, difficulty adjustable per
  traffic source)
- Private Access Token support (Apple device attestation, iOS 16.2+)

#### Challenge Pipeline
1. `getcaptcha` request sends encrypted telemetry
   - `n` field = answer to challenge
   - `c` field = challenge state (type, parameters)
2. Behavioral telemetry passed through obfuscated WASM VM
3. Proof-of-work computed (hashcash-style, WASM-based)
4. If confidence high: invisible passcode issued
5. If confidence low: visual challenge (image grid)
6. Passcode redeemed server-side via `siteverify`

#### Key Differentiator vs reCAPTCHA
- No Google account/cookie to correlate against
- Leans harder on proof-of-work tax (adjustable per traffic)
- Image challenges double as paid labeling work (business model)
- Private Access Token support for Apple devices

---

## 9. Complete Detection Vectors — Exhaustive Checklist

### 9.1 JS Property Checks

#### Automation Flags
- `navigator.webdriver` — true under automation, must be
  undefined/false
- `window.navigator` Proxy detection — check if `navigator` is wrapped
- `document.$chrome_asyncScriptInfo` — ChromeDriver artifact
- `window.__webdriver_evaluate` / `__webdriver_unwrapped`
- `window.__selenium_evaluate` / `__selenium_unwrapped`
- `window.__fxdriver_evaluate` / `__fxdriver_unwrapped`
- `window.__driver_evaluate` / `__driver_unwrapped`
- `window.__webdriver_script_fn` / `__webdriver_script_func`
- `window.__$webdriverAsyncExecutor`
- `window.__lastWatirAlert` / `__lastWatirConfirm` / `__lastWatirPrompt`
- `window.__phantomas`
- `window.__nightmare`
- `window._Selenium_IDE_Recorder`
- `window.calledPhantom` / `window.callSelenium`
- `window.domAutomationController`
- `window.awesomium`
- `window.geb`
- `window.fmget_targets`
- `window.OSMJIF`
- `window.spynner_additional_js_loaded`
- `window.watinExpressionError` / `window.watinExpressionResult`

#### ChromeDriver `cdc_` Properties
- `window.cdc_adoQpoasnfa76pfcZLmcfl_Array`
- `window.cdc_adoQpoasnfa76pfcZLmcfl_Object`
- `window.cdc_adoQpoasnfa76pfcZLmcfl_Promise`
- `window.cdc_adoQpoasnfa76pfcZLmcfl_Proxy`
- `window.cdc_adoQpoasnfa76pfcZLmcfl_Symbol`
- (Dynamic names per ChromeDriver version — search with regex:
  `/.+_.+_(Array|Promise|Symbol)/`)

#### Framework-Specific Globals
- Playwright: `window.__playwright__binding__`, `window.__pw_func`
- Puppeteer: `//# sourceURL=__puppeteer_evaluation_script__`
- Cypress: `window.Cypress`, `window.cy`
- Electron: `window.process`, `window.require`
- Nightmare: `window.__nightmare`
- PhantomJS: `window.callPhantom`, `window._phantom`

#### Chrome Object Properties
- `window.chrome` — must exist with specific nested structure
- `window.chrome.app` — must exist and be properly shaped
- `window.chrome.csi()` — must be a function
- `window.chrome.loadTimes()` — must be a function
- `window.chrome.runtime` — must exist with proper API surface
- Property descriptor integrity — `Object.getOwnPropertyDescriptor()`
  must match native shape
- `.toString()` output — must be `function csi() { [native code] }`

#### Navigator Properties
- `navigator.userAgent` — must not contain `HeadlessChrome`
- `navigator.plugins` — must have plausible length and content
- `navigator.mimeTypes` — must match plugins
- `navigator.languages` — must be populated and match locale
- `navigator.language` — must match `navigator.languages[0]`
- `navigator.platform` — must be plausible for claimed OS
- `navigator.vendor` — must be `Google Inc.` for Chrome
- `navigator.vendorSub` — must be empty string for Chrome
- `navigator.product` — must be `Gecko`
- `navigator.productSub` — must be `20030107` for Chrome
- `navigator.hardwareConcurrency` — plausible CPU core count
- `navigator.deviceMemory` — plausible RAM (Chrome only)
- `navigator.connection.effectiveType` / `rtt` / `downlink`
- `navigator.cookieEnabled`
- `navigator.doNotTrack`
- `navigator.maxTouchPoints`
- `navigator.mediaDevices.enumerateDevices()` — must return
  expected devices

#### Permissions API
- `navigator.permissions.query({name: 'notifications'})` — must
  not contradict `Notification.permission`
- `navigator.permissions.query({name: 'geolocation'})`
- `navigator.permissions.query({name: 'camera'})`
- `navigator.permissions.query({name: 'microphone'})`

#### Storage & Service Workers
- `navigator.serviceWorker` — should not have unexpected
  registrations
- `navigator.storage.estimate()` — quota inconsistency detection
- `window.indexedDB` — open without throwing
- `window.localStorage` / `window.sessionStorage` — available
  and writable
- `window.caches` — CacheStorage API availability

#### Battery & Device APIs
- `navigator.getBattery()` — should be available on laptops,
  absent in some headless
- `navigator.vibrate()` — present on mobile
- `navigator.wakeLock` — present on mobile, try/catch to detect
  headless

#### Client Hints
- `navigator.userAgentData.brands` — must match UA
- `navigator.userAgentData.mobile` — must be plausible
- `navigator.userAgentData.platform` — must match claimed OS
- `getHighEntropyValues(['architecture', 'bitness', 'model',
  'platformVersion', 'fullVersionList'])` — must return consistent
  values

### 9.2 WebGL Checks

#### Renderer & Vendor
- `UNMASKED_VENDOR_WEBGL` — must not be SwiftShader, llvmpipe, or
  Mesa unless claimed GPU is software
- `UNMASKED_RENDERER_WEBGL` — must match claimed hardware
- Renderer must not contain `SwiftShader`, `ANGLE (Google,` with
  software backend
- Known bad: `0x0000C0DE` (SwiftShader device ID)

#### GPU Parameters
- `MAX_TEXTURE_SIZE` — headless often has smaller/larger limits
- `MAX_VIEWPORT_DIMS`
- `MAX_RENDERBUFFER_SIZE`
- `MAX_VERTEX_ATTRIBS`
- `MAX_VERTEX_UNIFORM_VECTORS`
- `MAX_FRAGMENT_UNIFORM_VECTORS`
- `MAX_COMBINED_TEXTURE_IMAGE_UNITS`
- `MAX_CUBE_MAP_TEXTURE_SIZE`
- `ALIASED_POINT_SIZE_RANGE` / `ALIASED_LINE_WIDTH_RANGE`
- `SHADING_LANGUAGE_VERSION`

#### Extensions
- Full list of supported `WEBGL_` / `EXT_` / `OES_` extensions
- Cross-check against renderer claim (real GPU supports more
  extensions than software renders)

#### WebGL2 Context
- `WebGL2RenderingContext` availability
- WebGL2 parameter differences

#### Rendering Fingerprint
- Draw a controlled 3D scene, `readPixels()` output hash
- Shader precision differences (headless shaders differ)
- Anti-aliasing behavior
- Bezier curve aliasing patterns

### 9.3 Canvas Checks

#### Pixel Fingerprint
- Draw text + shapes + gradients, read pixel data via
  `toDataURL()` or `getImageData()`
- Text rendering differs by GPU, driver, font rasterizer, OS
- Subpixel positioning of text in canvas differs between headless
  and headful
- Hashed output should vary per GPU

#### Multiple Canvas Methods
- `toDataURL('image/png')`
- `toDataURL('image/jpeg')`
- `getImageData()` — raw pixel array
- Canvas 2D vs WebGL canvas vs OffscreenCanvas

#### Cross-Context Canvas
- Compare main thread Canvas vs Worker OffscreenCanvas output
- Spoofing tools often patch only main thread

#### Text & Emoji Rendering
- Canvas with emoji characters (rendering differs across OS)
- Canvas with specific Unicode text
- TextMetrics measurement (width, bounding box)

### 9.4 Audio Checks

#### Web Audio API
- OscillatorNode → AnalyserNode → Float32Array output
- Hardware-dependent timing differences
- Headless environments often have no audio stack or software-only
  fallback
- Sample rate differences

#### AudioContext Properties
- `audioContext.sampleRate`
- `audioContext.baseLatency`
- `audioContext.outputLatency`
- `audioContext.state` (must be 'running' after resume)

#### Codec Support
- `MediaSource.isTypeSupported()` — audio/video MIME types
- `HTMLMediaElement.canPlayType()` — codec support
- Headless often missing codecs: AAC, H.264, MP3, Opus

#### Speech Synthesis
- `window.speechSynthesis.getVoices()` — must return voices
- Headless often returns 0 voices
- Voice language and name enumeration

#### Media Devices
- `navigator.mediaDevices.enumerateDevices()` — headless has
  no audioinput/videoinput devices
- Device labels and kinds

### 9.5 Timing Checks

#### High-Resolution Timers
- `performance.now()` resolution (some environments clamp)
- `performance.timeOrigin`
- `performance.getEntriesByType('navigation')` timing
- `performance.getEntriesByType('paint')` timing
- `performance.getEntriesByType('resource')` — resource loading
  waterfall

#### requestAnimationFrame
- Cadence and jitter profile
- Headless: software-generated cadence, different jitter
- Headful: vsync-aligned, 60Hz typical

#### Execution Timing
- Time to collect and submit fingerprint signals (200-800ms real
  vs ~50ms bot)
- Script execution duration anomalies
- Event loop lag measurement

#### Date & Timer Precision
- `Date.now()` precision — Firefox RFP rounds to 100ms
- `setTimeout(fn, 0)` delay under load
- Timer clamping detection (CreepJS method: 10 delayed timestamps)

#### Paint & Load Timing
- `first-paint` / `first-contentful-paint` timing
- DOMContentLoaded / load event timing
- Headless rendering produces different paint timings than real
  page load

### 9.6 Network Checks

#### TLS Fingerprinting
- JA3 hash — cipher suites, TLS extensions, curves, formats
- JA4 hash — sorted + human-readable format: `t13d1516h2`
- JA4H — HTTP-level fingerprint (header order)
- JA4T — TCP-level fingerprint (SYN packet, TTL, options)
- GREASE presence (Chrome adds GREASE ciphers/extensions)

#### HTTP/2 Fingerprinting
- SETTINGS frame values (initial window size, max concurrent streams)
- Pseudo-header order (`:method`, `:scheme`, `:authority`, `:path`)
- Header order at HPACK level (browsers: deterministic; libs:
  insertion/alphabetical)
- WINDOW_UPDATE timing and values
- Stream priority and dependency ordering
- `PRIORITY` frame usage patterns

#### HTTP/3 (QUIC) Fingerprinting
- Transport parameters
- Initial flow control limits
- Connection ID length

#### HTTP Header Checks
- `User-Agent` — must be consistent with Sec-CH-UA Client Hints
- `Sec-CH-UA` / `Sec-CH-UA-Mobile` / `Sec-CH-UA-Platform` — all 3
  low-entropy hints must be present on Chrome
- `Sec-CH-UA-Full-Version-List` — must be present after Accept-CH
  challenge
- `Accept-Language` — must be populated
- `Accept-Encoding` — must include `br` (Brotli) for modern browsers
- `Sec-Fetch-Site` / `Sec-Fetch-Mode` / `Sec-Fetch-Dest` / `Sec-Fetch-User`
- Header case (Chrome sends lowercase standard headers)
- Header ordering (every browser version has stable order)

#### IP & ASN Checks
- IP geolocation must match timezone + language
- IP ASN — residential ISP vs datacenter vs hosting
- Proxy/VPN detection via IP databases
- Tor exit node detection

### 9.7 Behavioral Checks

#### Mouse Movement
- Event count (human: 50-500+ per page; bot: 0-5)
- Trajectory smoothness (human: curved with jitter; bot: straight
  lines or perfectly calculated curves)
- Acceleration profiles (human: ease-in/ease-out, Fitts's law;
  bot: instant start/stop)
- Micro-movements while hovering (human: 2-5px jitter; bot: perfectly
  still)
- Target overshoot and correction (human: slight overshoot; bot:
  precise targeting)
- Movement entropy (Shannon entropy of coordinate stream should be
  above threshold)
- Path distribution (human: scans page; bot: direct paths)
- Coordinate precision (human: sub-pixel; bot: integer pixels)

#### Touch Events
- Touch pressure variation (mobile)
- Contact area variation
- Multi-touch patterns
- Tilt angle data (when supported)

#### Keyboard Activity
- Key-down/key-up interval (human: 30-120ms typical; bot: identical
  or instant)
- Inter-key timing variance (human: variable per key pair; bot:
  constant)
- Modifier key patterns (Shift before/after letter)
- Form fill: character-by-character vs paste vs programmatic value
- Dwell time variance across keys

#### Scroll Behavior
- Scroll velocity profiles (human: momentum, variable; bot:
  constant or step-function)
- Scroll direction changes
- Relationship between scroll and content visibility
- Scroll event entropy
- Trackpad vs mouse-wheel patterns

#### Click Patterns
- Time between page load and first interaction
- Click duration
- Click position distribution
- Double-click intervals
- Presence of `mousemove` events preceding click

#### Form Interaction
- Field fill order (human: may tab around, edit out of order;
  bot: source-order traversal)
- Focus/blur event sequence
- Autocomplete/autofill patterns

#### Page Navigation
- Page dwell time distribution (human: continuous; bot: bimodal —
  ns or exact wait time)
- Navigation path diversity
- Referrer header consistency

#### Session Analysis
- Request rate and timing distributions
- Inter-request intervals
- Session duration
- Page depth and exploration patterns

#### Cross-Session Clustering
- Fingerprint distribution anomalies (500 identical fingerprints vs
  500 too-diverse fingerprints)
- Shared operational cadence across sessions
- Common configuration residue

### 9.8 Chrome Extension / Plugin Checks

#### Extension Probes
- **Akamai 60-extension probe**: fetch known `chrome-extension://`
  URLs — real users have some installed, headless has none
- Known extension IDs probed: uBlock Origin, LastPass, Bitwarden,
  1Password, AdBlock, AdBlock Plus, Honey, Grammarly, etc.
- Result: 60/60 failures = statistically impossible for real user

#### Plugin Array
- `navigator.plugins.length` should be > 0
- Plugin names should match Chrome: Chrome PDF Plugin,
  Chrome PDF Viewer, Native Client
- Plugin MIME types should match names

#### `domBlockers` Detection
- Inject elements with class names matching ad-blocker filter lists
- Detect which elements are hidden
- Presence of specific blockers is fingerprintable

### 9.9 Font Checks

#### Font Enumeration
- System font list via font measuring (render text, measure width)
- Stock Windows 11 has specific Microsoft fonts
- macOS Sonoma has Apple system fonts
- Linux distros vary widely
- Headless/server environments have minimal fonts

#### Font Fingerprint Vectors
- Number of installed fonts (real: 100-500+; server: 10-50)
- Specific font names and families
- Font rendering differences (hinting, anti-aliasing)
- Unicode character rendering

### 9.10 Screen & Display Checks

#### Screen Properties
- `screen.width` / `screen.height`
- `screen.availWidth` / `screen.availHeight`
- `screen.colorDepth` / `screen.pixelDepth`
- `screen.orientation` / `screen.orientation.type`

#### Window Properties
- `window.innerWidth` / `window.innerHeight`
- `window.outerWidth` / `window.outerHeight`
- `window.screenX` / `window.screenY`
- `window.screenLeft` / `window.screenTop`
- `window.devicePixelRatio`
- `window.mozInnerScreenY` (non-standard, may betray spoofing)

#### Viewport Checks
- Default headless: 800x600, 1280x720
- Taskbar presence: `screen.availHeight < screen.height`
- Screen frame offset: `window.screenTop > 0` in normal usage
- Multi-monitor: `screenX` can be negative or > 0

#### Media Queries
- `matchMedia('(color-gamut: srgb)')`
- `matchMedia('(color-gamut: p3)')`
- `matchMedia('(color-gamut: rec2020)')`
- `matchMedia('(dynamic-range: standard)')` / `'(dynamic-range: high)'`
- `matchMedia('(inverted-colors)')`
- `matchMedia('(pointer: fine)')` / `'(pointer: coarse)'`
- `matchMedia('(hover: hover)')` / `'(hover: none)'`
- `matchMedia('(prefers-color-scheme: dark)')` / `'(light)'`
- `matchMedia('(prefers-reduced-motion: reduce)')`
- `matchMedia('(prefers-contrast: more)')`
- `matchMedia('(forced-colors: active)')`
- `matchMedia('(display-mode: browser)')` / `'(standalone)'` / `'(fullscreen)'`

### 9.11. CDP / DevTools Protocol Detection

#### Runtime.enable Side Effect
- Classic technique: `console.debug` with getter that throws
- When CDP `Runtime.enable` is active, error.stack serialization
  triggers getter
- Detected via timing gap (~0.3ms) or observable behavior difference
- Chrome patched the best-known variant in August 2025
- Newer variants exploit different CDP side effects

#### DevTools UI Detection
- `window.devtools` property presence (some versions)
- Element inspection timing differences
- Console element access patterns

#### Injected Script Detection
- `Page.addScriptToEvaluateOnNewDocument` residue
- `Runtime.evaluate` call patterns
- CDP domain enumeration

#### Protocol-Specific Artifacts
- WebSocket frames from CDP connection
- CDP command timing patterns
- Evaluation script source URL annotations (Puppeteer's
  `__puppeteer_evaluation_script__`)

### 9.12. CSS / Rendering Engine Checks

#### CSS Computed Styles
- Specific properties expected per render engine
- Box model differences
- `getComputedStyle()` output consistency

#### SVG Rendering
- SVG filter output differences
- Text rendering in SVG
- ForeignObject behavior

#### Math Operations
- Floating-point operation precision differences between engines
- Trigonometric function output (Math.sin, Math.cos)
- CreepJS: JS Runtime (Math) fingerprint

#### DOM Operation Timing
- `document.createElement` timing
- `appendChild` / `insertBefore` timing
- Attribute access patterns

### 9.13. Obfuscation & Anti-Tamper Checks

#### Function Integrity
- `Function.prototype.toString` must return `function foo() { [native code] }`
  for all native functions
- Function arity (`.length` property) must match spec
- `Function.prototype.call` / `apply` / `bind` integrity

#### Prototype Chain Integrity
- `Object.getPrototypeOf(nativeFunc)` must be `Function.prototype`
- No unexpected properties on native prototypes
- Property descriptors must match native (configurable, enumerable,
  writable flags)

#### Proxy Detection
- Referential equality across repeated access (native: same object;
  Proxy: may return different wrapper)
- `Proxy` handler trap detection via `instanceof` and internal slots
- `ToPropertyKey` behavior differences

#### Getter/Setter Detection
- Check if property is a getter vs direct value
- Getter should not be a Proxy
- `.toString()` output of getter

### 9.14. VM & Environment Checks

#### Virtualization Detection
- `navigator.deviceMemory` vs `hardwareConcurrency` vs OS claim
- WebGL renderer matching VM GPU (VMware, VirtualBox, Parallels)
- Timing anomalies (VM execution slower than native)
- Screen resolution common in VMs
- Font set inconsistencies

#### Battery API
- `navigator.getBattery()` — present on real laptops, absent in
  many VMs/servers

#### Temperature & Hardware Sensors
- `navigator.hid` / `navigator.usb` / `navigator.bluetooth` —
  availability varies
- `navigator.serial` — present in some configurations
- `navigator.mediaCapabilities` — decoding info

#### Platform Consistency Checks
- OS from User-Agent must match `navigator.platform`
- OS from User-Agent must match TLS fingerprint JA4
- OS from User-Agent must match TCP fingerprint (TTL, options)
- OS from User-Agent must match Client Hints platform
- Timezone must be consistent with IP geolocation

### 9.15. Cross-Layer Coherence (The Ultimate Check)

The most sophisticated detection systems don't check any single
signal. They check **agreement across independent layers**:

| Layer 1 Claim | Layer 2 Signal | Layer 3 Signal |
|--------------|----------------|----------------|
| Chrome 126 Windows | JA4: Chrome browser TLS | TCP TTL ~128 (Windows) |
| Chrome 126 Windows | Sec-CH-UA: "Chrome" v="126" | WebGL: ANGLE (NVIDIA) |
| Chrome 126 Windows | Accept-Language: en-US | Timezone: America/New_York |
| Chrome 126 Windows | IP: Residential ASN | IP geolocation: US East |

If any combination is **contradictory** — e.g., TLS says Linux,
headers say Windows — the session is flagged regardless of how clean
each individual signal looks.

### Detection Vendor Signal Comparison

| Signal | CF | Akamai | DataDome | PX | hCaptcha |
|--------|----|--------|----------|----|----------|
| TLS fingerprint | H | Y | H | Y | - |
| HTTP/2 fingerprint | Y | Y | H | Y | - |
| Canvas fingerprint | - | H | Y | H | - |
| WebGL renderer | - | Y | Y | Y | - |
| Screen resolution | - | Y | Y | Y | - |
| Timezone/language | Y | Y | Y | Y | - |
| Installed plugins | - | Y | - | Y | - |
| Mouse/touch events | - | Y | Y | H | Y |
| Timing patterns | Y | Y | Y | H | Y |
| DOM manipulation | Y | - | - | Y | - |
| CDP/automation flags | Y | Y | Y | H | - |
| IP reputation | H | Y | H | Y | H |
| UA consistency | H | Y | Y | Y | - |

Legend: Y = checked, H = heavily weighted, - = not checked

---

## Key Sources

1. FingerprintJS — open-source v5 source tree, Pro Smart Signals docs
2. CreepJS — GitHub repo, `src/lies/`, `src/headless/`, `src/resistance/`
3. BotD — GitHub README, FingerprintJS docs
4. Cloudflare — "JavaScript Detections" docs, "Precursor" announcement,
   "Bot detection engines" docs, Turnstile docs
5. Akamai — Reverse-engineering teardowns (512KB sensor analysis,
   field enumeration, v3 VM analysis), Scrapfly/Scrappey guides
6. DataDome — "How New Headless Chrome & the CDP Signal Are Impacting
   Bot Detection," "HTTP/2 fingerprinting" research,
   reverse-engineering teardowns
7. PerimeterX — ProxyHat analysis, ddactic.net 20-vendor
   reverse-engineering
8. reCAPTCHA v3 — GitHub reverse-engineering (vm/reload/fingerprint),
   CaptchaAI analysis, Google docs
9. hCaptcha — Challenge pipeline analysis, crawlex.net comparison,
   ddactic.net vendor survey
10. Foil / Sentinel / cside — Headless detection research 2026
11. WebDecoy — "Browser Fingerprinting 2026: What Works, What Doesn't"
12. Crawlex.net — "FingerprintJS internals," "Akamai sensor data," etc.
