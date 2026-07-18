# HTTP header & protocol fingerprint reference

Comprehensive technical reference for Chrome 136+ on Windows 10/11 (2026). Exact
header values, order, protocol frames, and behavioral details needed for accurate
browser fingerprint emulation.

---

## 1. Complete HTTP request header order (HTTP/2)

Chrome sends headers in this exact order over HTTP/2 (lowercase as required by
the protocol). The order is itself a fingerprint signal (JA4H).

```
sec-ch-ua
sec-ch-ua-mobile
sec-ch-ua-platform
upgrade-insecure-requests
user-agent
accept
sec-fetch-site
sec-fetch-mode
sec-fetch-user
sec-fetch-dest
accept-encoding
accept-language
priority
```

HTTP/2 pseudo-headers precede these, in this order:

```
:method
:authority
:scheme
:path
```

### HTTP/1.1 header order

Over HTTP/1.1 the order is the same but with `Host` added (usually after the
request line or between pseudo-headers and the rest). `Connection` may also
appear (see §7). Chrome does **not** send `Connection` or `Keep-Alive` on
HTTP/2 or HTTP/3.

---

## 2. Exact header values by request type

### Navigation (top-level document)

```
:method: GET
:authority: example.com
:scheme: https
:path: /
sec-ch-ua: "Google Chrome";v="136", "Chromium";v="136", "Not/A)Brand";v="24"
sec-ch-ua-mobile: ?0
sec-ch-ua-platform: "Windows"
upgrade-insecure-requests: 1
user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36
accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7
sec-fetch-site: none
sec-fetch-mode: navigate
sec-fetch-user: ?1
sec-fetch-dest: document
accept-encoding: gzip, deflate, br, zstd
accept-language: en
priority: u=0, i
```

### Same-origin navigation (clicking a link)

```
sec-fetch-site: same-origin
sec-fetch-mode: navigate
sec-fetch-user: ?1
sec-fetch-dest: document
```

### Images (`<img>`)

```
accept: image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8
sec-fetch-site: same-origin  (or cross-site)
sec-fetch-mode: no-cors
sec-fetch-dest: image
priority: u=5, i
```

### CSS (`<link rel="stylesheet">`)

```
accept: text/css,*/*;q=0.1
sec-fetch-site: same-origin  (or cross-site)
sec-fetch-mode: no-cors
sec-fetch-dest: style
priority: u=0
```

### JavaScript (`<script>`)

```
accept: */*
sec-fetch-site: same-origin  (or cross-site)
sec-fetch-mode: no-cors
sec-fetch-dest: script
priority: u=1
```

### Fonts (`@font-face`)

```
accept: */*
sec-fetch-site: same-origin  (or cross-site)
sec-fetch-mode: cors
sec-fetch-dest: font
priority: u=2
```

### XHR / `fetch()` API

```
accept: */*
sec-fetch-site: same-origin  (or cross-site)
sec-fetch-mode: cors  (or same-origin, no-cors)
sec-fetch-dest: empty
priority: u=3  (default)
```

### `<iframe>` navigation

```
sec-fetch-dest: iframe
sec-fetch-mode: navigate
sec-fetch-user: ?1
```

### WebSocket upgrade

```
sec-fetch-mode: websocket
sec-fetch-dest: empty
upgrade: websocket
connection: Upgrade
```

### `signed-exchange` (`application/signed-exchange;v=b3`)

Chrome appends `application/signed-exchange;v=b3;q=0.7` to the navigation
`Accept` header. This is the SXG (Signed HTTP Exchange) format.

---

## 3. `Sec-CH-UA` (User-Agent Client Hints)

### Low-entropy (sent on every request, no server opt-in needed)

| Header | Value (desktop Windows) | Notes |
|--------|--------------------------|-------|
| `Sec-CH-UA` | `"Google Chrome";v="136", "Chromium";v="136", "Not/A)Brand";v="24"` | The GREASE entry (`Not/A)Brand` or `Not;A=Brand`) uses a random-looking token and version that changes per Chrome build. The escaping varies: Chrome uses `Not/A)Brand` or `Not;A=Brand` depending on version. |
| `Sec-CH-UA-Mobile` | `?0` | `?1` on mobile, `?0` on desktop |
| `Sec-CH-UA-Platform` | `"Windows"` | `"macOS"`, `"Linux"`, `"Android"`, `"iOS"`, `"Chrome OS"` |

### High-entropy (requires `Accept-CH` server opt-in)

These are only sent after the server requests them with `Accept-CH` in a
response header. Do not send them unprompted — that itself is a fingerprint.

| Header | Example value (desktop) | Notes |
|--------|--------------------------|-------|
| `Sec-CH-UA-Full-Version` | `"136.0.7103.0"` | Deprecated but still honored. Prefer `Full-Version-List`. |
| `Sec-CH-UA-Full-Version-List` | `"Google Chrome";v="136.0.7103.0", "Chromium";v="136.0.7103.0", "Not/A)Brand";v="24.0.0.0"` | Replaces `Full-Version`. |
| `Sec-CH-UA-Platform-Version` | `"15.0.0"` | Windows build number. Win 11 24H2 = `15.0.0`, Win 10 22H2 = `10.0.19045`. |
| `Sec-CH-UA-Arch` | `"x86"` | `"x86"`, `"arm"`, `"x86_64"` is not sent; Chrome uses `"x86"` even on x64 on Windows. |
| `Sec-CH-UA-Bitness` | `"64"` | `"32"` or `"64"` |
| `Sec-CH-UA-Model` | `""` (empty string) | Desktop sends `""`. Mobile sends the device model like `"Pixel 9 Pro"`. |
| `Sec-CH-UA-WoW64` | `?0` | Boolean. `?1` if running under WoW64 (32-bit Chrome on 64-bit Windows). Modern Chrome is 64-bit, so `?0`. |

### Per-request `Accept-CH` policy

Chrome sends low-entropy hints unconditionally. High-entropy hints are cached
per origin after the server requests them via `Accept-CH` response header. The
browser does **not** send high-entropy hints on the first request to an origin.

---

## 4. `Sec-Fetch-*` headers

These four headers form the **Fetch Metadata** spec. All are sent on every
request (over HTTPS). They never appear on HTTP.

### `Sec-Fetch-Site`

| Value | Meaning |
|-------|---------|
| `none` | User directly navigated (typed URL, bookmark, new tab) |
| `same-origin` | Same scheme + host + port as the page |
| `same-site` | Same registrable domain, different subdomain/scheme/port |
| `cross-site` | Different registrable domain |

Navigation: `none` on first navigation, `same-origin` for internal links,
`cross-site` for external links.

### `Sec-Fetch-Mode`

| Value | Usage |
|-------|-------|
| `navigate` | Top-level or iframe document navigation |
| `same-origin` | Same-origin fetch/XHR |
| `cors` | Cross-origin fetch/XHR with CORS |
| `no-cors` | `<img>`, `<script>`, `<link rel="stylesheet">`, `<video>`, `<audio>` |
| `websocket` | WebSocket upgrades |
| `nested-navigate` | `<iframe>` navigation (legacy; now `navigate` with `dest: iframe`) |

### `Sec-Fetch-Dest`

| Value | Resource type |
|-------|---------------|
| `document` | Top-level navigation |
| `iframe` | `<iframe>` navigation |
| `frame` | `<frame>` navigation (obsolete) |
| `script` | `<script>` |
| `style` | `<link rel="stylesheet">` |
| `image` | `<img>`, `background-image`, `<picture>` sources |
| `font` | `@font-face` |
| `empty` | `fetch()`, XHR, `navigator.sendBeacon()`, WebSocket, EventSource |
| `worker` | `new Worker()` |
| `sharedworker` | `new SharedWorker()` |
| `serviceworker` | `navigator.serviceWorker.register()` |
| `audioworklet` | AudioWorklet |
| `paintworklet` | CSS Paint API |
| `manifest` | `<link rel="manifest">` |
| `xslt` | XSLT transformations (rare) |
| `embed` | `<embed>` |
| `object` | `<object>` |
| `report` | CSP report, NEL report |

### `Sec-Fetch-User`

| Value | Meaning |
|-------|---------|
| `?1` | User-initiated navigation (click, form submit, address bar) |
| (absent) | Not a user-initiated navigation |

Only present when `Sec-Fetch-Mode` is `navigate` **and** the navigation was
triggered by a user activation. Always `?1` when present.

---

## 5. `Accept-Encoding`

Chrome 136+ on desktop:

```
Accept-Encoding: gzip, deflate, br, zstd
```

- `gzip` — universal, always first.
- `deflate` — legacy, infrequently used by servers.
- `br` — Brotli, supported since Chrome 50.
- `zstd` — Zstandard, supported since Chrome 123+. Send `zstd` as the last
  entry. Some fingerprinting systems track whether `zstd` is present.

Chrome 123+ added `zstd` support. Servers respond with `Content-Encoding: zstd`
if they choose it.

When shared dictionary compression is active, Chrome appends `dcb` (Brotli
dictionary) and `dcz` (Zstandard dictionary) tokens:

```
Accept-Encoding: gzip, deflate, br, zstd, dcb, dcz
```

This is rare and requires a prior `Use-As-Dictionary` response. Do not send
`dcb`/`dcz` unless you're emulating a connection with cached dictionaries.

---

## 6. `Accept-Language`

### Chrome 136+ (behavior change)

Starting in Chrome 136, the `Accept-Language` header was reduced to a single
language to reduce fingerprinting surface:

```
Accept-Language: en
```

Previously Chrome sent:

```
Accept-Language: en-US,en;q=0.9
```

Or for multilingual users:

```
Accept-Language: fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7
```

The exact value depends on the OS language settings. Common patterns:
- Single language (new default): `en`, `fr`, `de`, `ja`, `zh`
- With region variant: `en-US`, `en-GB`, `pt-BR`, `zh-CN`
- The `q=` weight parameter follows standard HTTP quality values.

**Anti-fingerprinting note:** The reduction to a single language tag is part of
Chrome's ongoing User-Agent reduction. The old multilanguage format is still
sent by older Chrome versions (pre-136).

---

## 7. Special headers

### `Accept-Post`, `Accept-Patch`

Chrome does **not** send `Accept-Post` or `Accept-Patch` on any request. These
are response-only headers that servers use to advertise supported media types
for POST/PATCH methods. Some browsers may send them in very specific contexts
(linked-data APIs), but Chrome mainstream does not.

### `Upgrade-Insecure-Requests`

Sent on every navigation request:

```
Upgrade-Insecure-Requests: 1
```

This tells the server the client prefers HTTPS. Absent on subresource requests
(images, CSS, JS, fonts).

### `Priority`

Sent on every request since Chrome adopted RFC 9218:

```
Priority: u=0, i     (CSS, critical resources)
Priority: u=1        (scripts)
Priority: u=2        (fonts)
Priority: u=3, i     (fetch/XHR default)
Priority: u=4, i     (images, media)
Priority: u=5, i     (images below the fold)
Priority: u=6        (prefetch)
Priority: u=7        (background sync, analytics)
```

- `u=N` — urgency from 0 (highest) to 7 (lowest). Default is 3.
- `i` — incremental flag. When present, the response can be processed
  incrementally (partial data is useful before complete download). Images,
  HTML, video are incremental. CSS, fonts, JSON are not.
- This header is **only** sent on HTTP/2 and HTTP/3 connections. Chrome does
  **not** send it on HTTP/1.1.

### `Sec-GPC`

Chrome sends this when the user has enabled Global Privacy Control:

```
Sec-GPC: 1
```

Rare — only present if the user has explicitly opted out of data sharing in
Chrome settings.

### `DNT`

Deprecated. Chrome removed `DNT` (Do Not Track) support. Do not send.

### `Save-Data`

```
Save-Data: on
```

Only sent when the user has enabled "Data Saver" mode in Chrome. Omit for
normal desktop traffic.

---

## 8. `Connection` and `Keep-Alive`

### HTTP/1.1

Chrome sends these on HTTP/1.1 connections:

```
Connection: keep-alive
```

This is technically redundant (HTTP/1.1 defaults to persistent connections) but
Chrome sends it for backward compatibility with misconfigured proxies and
load balancers. The `Keep-Alive` header is **not** sent by Chrome; it only
appears in server responses.

### HTTP/2 and HTTP/3

`Connection` and `Keep-Alive` are **forbidden** in HTTP/2 and HTTP/3. Chrome
does not send them and ignores them in responses. This is per RFC 9113 §8.2.2.

---

## 9. `Cache-Control` variations

### Default request Cache-Control

Chrome sends a default `Cache-Control` on **all** requests:

```
Cache-Control: no-cache
```

This means "I'll accept a cached response but only after revalidating with the
server." It is **not** `no-store` — the browser will cache if allowed.

### Variation by request type

| Resource type | Typical Cache-Control sent by Chrome |
|---------------|--------------------------------------|
| Navigation (initial) | `max-age=0` (may omit on first visit) |
| Navigation (back/forward) | (none — bfcache, no request sent) |
| Navigation (reload) | `no-cache` |
| Navigation (hard reload, Ctrl+F5) | `no-cache, no-store, must-revalidate` |
| `<img>` | (none — uses server Cache-Control) |
| `<link rel="stylesheet">` | (none) |
| `<script>` | (none) |
| `fetch()` / XHR | (none by default; `no-cache` if `cache: "no-cache"`) |

The browser does **not** send `Cache-Control` on most subresource requests — it
relies on the server's `Cache-Control` response header instead. When Chrome
does send it:

- **Reload** (`F5` / refresh button): `Cache-Control: no-cache` plus
  `Pragma: no-cache` (legacy, HTTP/1.1 only).
- **Hard reload** (`Ctrl+F5` / DevTools "Empty Cache and Hard Reload"):
  `Cache-Control: no-cache, no-store, must-revalidate` plus `Pragma: no-cache`.
- **History navigation** (back/forward): no `Cache-Control` at all — Chrome uses
  bfcache and may serve from cache without a network request. This is a common
  fingerprinting pitfall: `Cache-Control: no-cache` does **not** prevent
  bfcache from restoring the page without a request.

### `if-modified-since` / `if-none-match`

Chrome sends conditional headers based on the cached `Last-Modified` or `ETag`
from the previous response. These are standard HTTP caching. Chrome does not
send them on every request — only when it has a cached value to revalidate.

---

## 10. `Cookie` header format

Chrome sends the `Cookie` header in this format:

```
Cookie: name1=value1; name2=value2; name3=value3
```

- Cookies are separated by `; ` (semicolon + space).
- Values are URL-encoded as needed (Chrome percent-encodes special characters).
- Cookies are sent in order of creation (oldest first).
- `__Secure-` and `__Host-` prefixed cookies are only sent over HTTPS.
- `SameSite=Lax` is the default for cookies without an explicit `SameSite`
  attribute (since Chrome 80).
- `SameSite=None; Secure` is required for cross-site cookies.
- Chrome may omit the `Cookie` header entirely on first-party requests when no
  cookies exist for the domain.

Chrome does **not** send individual cookie attributes (`Domain`, `Path`,
`Expires`, `Secure`, `HttpOnly`, `SameSite`) — those are `Set-Cookie` response
headers only.

---

## 11. HTTP/2 connection preface

### Connection preface (PRI \* HTTP/2.0)

Every HTTP/2 connection starts with this 24-byte ASCII magic string:

```
PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n
```

Hex: `0x505249202a20485454502f322e300d0a0d0a534d0d0a0d0a`

### SETTINGS frame (Chrome 136)

Immediately after the preface, Chrome sends a SETTINGS frame with these
parameters (in this specific order):

| Setting ID | Name | Chrome 136 value | Meaning |
|------------|------|------------------|---------|
| 0x1 | `SETTINGS_HEADER_TABLE_SIZE` | 65536 (0x10000) | HPACK header table size (64 KB) |
| 0x2 | `SETTINGS_ENABLE_PUSH` | 0 | Server push disabled |
| 0x3 | `SETTINGS_MAX_CONCURRENT_STREAMS` | 1000 (varies; often 100 or 1000) | Max concurrent streams |
| 0x4 | `SETTINGS_INITIAL_WINDOW_SIZE` | 6291456 (0x600000) | Initial stream flow-control window (~6 MB) |
| 0x6 | `SETTINGS_MAX_HEADER_LIST_SIZE` | 262144 (0x40000) | Max uncompressed header size (256 KB) |

Notable: Chrome does **not** send `SETTINGS_MAX_FRAME_SIZE` (0x5), using the
default 16384 bytes.

### WINDOW_UPDATE frame

Immediately after SETTINGS, Chrome sends a connection-level WINDOW_UPDATE:

```
Window Size Increment: 15663105
```

This brings the total connection flow-control window to 15663105 + 65535
(default) = 15728640.

### Frame order in Chrome's HTTP/2 connection preface

1. Magic 24-byte preface (`PRI * HTTP/2.0...`)
2. SETTINGS frame (with the 4 parameters above)
3. WINDOW_UPDATE frame (15663105)
4. (optional) PRIORITY frames for stream prioritization tree
5. HEADERS frame (first request, containing pseudo-headers + regular headers)

### Akamai HTTP/2 fingerprint string

Chrome's HTTP/2 fingerprint expressed in Akamai format:

```
1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p
```

Broken down:
- `1:65536;2:0;4:6291456;6:262144` — SETTINGS frame (header table size,
  enable push, initial window size, max header list size)
- `15663105` — WINDOW_UPDATE increment
- `0` — priority scheme (0 = RFC 7540 priority)
- `m,a,s,p` — pseudo-header order: `:method`, `:authority`, `:scheme`, `:path`

### HTTP/2 vs HTTP/1.1 key differences for fingerprinting

| Feature | HTTP/1.1 | HTTP/2 |
|---------|----------|--------|
| Request line | `GET /path HTTP/1.1` | `:method` + `:path` pseudo-headers |
| Host | `Host: example.com` | `:authority` pseudo-header |
| Headers | Mixed/sent as-is | Lowercase, HPACK-compressed |
| Header order | Variable (but Chrome uses fixed order) | Fixed per implementation |
| `Connection` | Sent (`keep-alive`) | Forbidden |
| `Keep-Alive` | Not sent by Chrome | Forbidden |
| `Priority` header | Not sent | Sent (RFC 9218) |

---

## 12. TLS fingerprint notes

Chrome's TLS handshake (JA3/JA4) is a separate but related fingerprint surface.
Key characteristics:

- TLS 1.3 preferred (TLS_AES_128_GCM_SHA256 and TLS_AES_256_GCM_SHA384).
- GREASE cipher suites and extensions (random-looking values to prevent
  ossification).
- `supported_versions` includes 0x0304 (TLS 1.3), 0x0303 (TLS 1.2).
- Extension order includes `application_layer_protocol_negotiation` (ALPN)
  advertising `h2` and `http/1.1`.
- `key_share` for x25519 and secp256r1 (P-256).
- Chrome randomizes some TLS extension ordering per connection, but the
  SETTINGS frame and header order remain stable per version.

---

## 13. Resource-type header matrix

Quick reference table for the most common request types:

| Resource | Accept | Sec-Fetch-Mode | Sec-Fetch-Dest | Priority | Upgrade-Insecure-Requests |
|----------|--------|----------------|----------------|----------|---------------------------|
| Document (nav) | `text/html,...` | `navigate` | `document` | `u=0, i` | `1` |
| iframe nav | `text/html,...` | `navigate` | `iframe` | `u=0, i` | `1` |
| CSS | `text/css,*/*;q=0.1` | `no-cors` | `style` | `u=0` | — |
| JS | `*/*` | `no-cors` | `script` | `u=1` | — |
| Image | `image/avif,image/webp,...` | `no-cors` | `image` | `u=5, i` | — |
| Font | `*/*` | `cors` | `font` | `u=2` | — |
| fetch/XHR | `*/*` | `cors`/`same-origin` | `empty` | `u=3`/`u=3, i` | — |
| WebSocket | (none/binary) | `websocket` | `empty` | — | — |
| Manifest | `*/*` | `same-origin` | `manifest` | `u=3` | — |
| Prefetch | `*/*` | `no-cors` | `empty` | `u=6` | — |

---

## References

- RFC 9113: HTTP/2 (obsoletes RFC 7540)
- RFC 9218: Extensible Prioritization Scheme for HTTP
- Fetch Metadata Request Headers (W3C): Sec-Fetch-\*
- UA Client Hints (WICG): Sec-CH-UA-\*
- Chrome Platform Status / chromestatus.com
- ScrapFly HTTP/2 Fingerprint tool: https://tools.scrapfly.io/api/fp/akamai
- bogdanfinn/tls-client browser profile definitions
