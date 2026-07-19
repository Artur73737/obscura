<p align="center">
  <img src="assets/logo.svg" alt="Obscura" width="80" />
</p>

<h2 align="center">Obscura</h2>

<p align="center">
  <strong>The open-source headless browser for AI agents and web scraping.</strong><br>
  Lightweight, stealthy, and built in Rust.
</p>

---

Obscura is a headless browser engine written in Rust, built for web scraping and AI agent automation. It runs real JavaScript via V8, supports the Chrome DevTools Protocol, and acts as a drop-in replacement for headless Chrome with Puppeteer and Playwright.

### Why Obscura over headless Chrome?

Designed for automation at scale, not desktop browsing.

| Metric       | Obscura      | Headless Chrome |
|--------------|--------------|------------------|
| Memory       | **30 MB**    | 200+ MB          |
| Binary size  | **70 MB**    | 300+ MB          |
| Anti-detect  | **Built-in** | None          |
| Page load    | **85 ms**    | ~500 ms          |
| Startup      | **Instant**  | ~2s              |
| Puppeteer    | **Yes**      | Yes              |
| Playwright   | **Yes**      | Yes              |



## Install

### Download

Grab the latest binary from [Releases](https://github.com/h4ckf0r0day/obscura/releases):

```bash
# Linux x86_64
curl -LO https://github.com/h4ckf0r0day/obscura/releases/latest/download/obscura-x86_64-linux.tar.gz
tar xzf obscura-x86_64-linux.tar.gz
./obscura fetch https://example.com --eval "document.title"

# Linux ARM64 (aarch64)
curl -LO https://github.com/h4ckf0r0day/obscura/releases/latest/download/obscura-aarch64-linux.tar.gz
tar xzf obscura-aarch64-linux.tar.gz

# Arch Linux (AUR)
yay -S obscura-browser

# macOS Apple Silicon
curl -LO https://github.com/h4ckf0r0day/obscura/releases/latest/download/obscura-aarch64-macos.tar.gz
tar xzf obscura-aarch64-macos.tar.gz

# macOS Intel
curl -LO https://github.com/h4ckf0r0day/obscura/releases/latest/download/obscura-x86_64-macos.tar.gz
tar xzf obscura-x86_64-macos.tar.gz

# Windows
Download the `.zip` from the releases page and extract it manually.
```

No Chrome, no Node.js, no dependencies. Release archives include both
`obscura` and `obscura-worker`; keep them in the same directory for the
parallel `scrape` command.

Linux release builds target Ubuntu 22.04 so the downloaded binary remains
usable on common LTS servers with glibc 2.35+.

### Docker

```bash
docker run -d --name obscura -p 127.0.0.1:9222:9222 h4ckf0r0day/obscura
```

Image on [Docker Hub](https://hub.docker.com/r/h4ckf0r0day/obscura). Multi-stage build on `distroless/cc`, no shell, no package manager, ~57 MB compressed.

### Build from source

```bash
git clone https://github.com/h4ckf0r0day/obscura.git
cd obscura
cargo build --release

# With stealth mode (anti-detection + tracker blocking)
cargo build --release --features stealth
```

Requires Rust 1.75+ ([rustup.rs](https://rustup.rs)). First build takes ~5 min (V8 compiles from source, cached after).

## Quick Start

### Fetch a page

```bash
# Get the page title
obscura fetch https://example.com --eval "document.title"

# Extract all links
obscura fetch https://example.com --dump links

# Render JavaScript and dump HTML
obscura fetch https://news.ycombinator.com --dump html

# Write dump or eval output to a file
obscura fetch https://example.com --dump text --output page.txt

# Stream the raw response body verbatim (binary-safe; bypasses the JS/DOM layer).
# Use this for images, JSON, JS, CSS, or any non-HTML resource.
obscura fetch https://picsum.photos/200/300 --dump original > photo.jpg

# List every sub-resource URL the page would fetch (NDJSON; one record per asset)
obscura fetch https://example.com --dump assets

# Fetch through an HTTP or SOCKS proxy
obscura --proxy socks5://127.0.0.1:1080 fetch https://example.com --dump text

# Wait for dynamic content
obscura fetch https://example.com --wait-until networkidle0

# Bound navigation time for slow or broken pages
obscura fetch https://example.com --timeout 10
```

### Start the CDP server

```bash
obscura serve --port 9222

# With stealth mode (anti-detection + tracker blocking)
obscura serve --port 9222 --stealth
```

### Scrape in parallel

```bash
obscura scrape url1 url2 url3 ... \
  --concurrency 25 \
  --eval "document.querySelector('h1').textContent" \
  --format json

# Suppress scrape progress on stderr for script-friendly output
obscura scrape https://example.com --quiet --format json

# Scrape workers inherit the global proxy
obscura --proxy http://127.0.0.1:8080 scrape https://example.com https://news.ycombinator.com
```

### Web search

```bash
# Search DuckDuckGo
obscura search "news iran" --engine duckduckgo --max-results 20

# Scrape each result page content
obscura search "rust tokio" --depth page --scrape text --format ndjson -o results.ndjson

# Google captcha? Auto-fallback to DuckDuckGo
obscura search "news" --engine google --fallback duckduckgo

# Reuse a persistent, warm browser session (real cookies carry across runs)
obscura search "rust jobs" --engine bing --session ~/.obscura/session

# Expose search over HTTP (POST /search) and WebSocket
obscura octo-serve --port 8080
```

When an engine serves a wall instead of results, the error names the **captcha
type** (reCAPTCHA v2/v3, hCaptcha, Google "unusual traffic", Cloudflare, consent
wall) and the **likely reason** it triggered (IP reputation, low behavioral
score, request velocity, regional consent) with how to recover.

### Warm sessions

A persisted `--session DIR` keeps the browser's cookie jar between runs, so an
engine sees a returning visitor with real history rather than a cold client — a
legitimate trust signal that tends to raise anti-bot scores. Nothing is
fabricated: it is the same jar Obscura builds while actually browsing.
`--session` works on `search`, `monitor`, and `octo-serve`.

```bash
# Mature a session by really browsing an engine for 15 minutes, then save it
obscura warmup --engine bing --minutes 15 --session ~/.obscura/session

# Optionally seed with your own queries
obscura warmup --engine duckduckgo --minutes 5 --session ./sess --query "rust jobs"

# Or also visit specific target sites to mature their cookies too
obscura warmup --minutes 10 --session ./sess --url https://example.com --url https://news.ycombinator.com

# Then reuse it anywhere
obscura search "..." --engine bing --session ~/.obscura/session

# Inspect a session without opening cookies.json
obscura session-info --session ~/.obscura/session
```

### Monitor pages for changes

```bash
# Watch a page every 60s, print changes as NDJSON
obscura monitor https://example.com/status

# Watch with selector, condition, and custom output
obscura monitor https://x.com/page --selector "article:first-child" \
    --condition "textContent.includes('2026')" \
    --on-change "JSON.stringify({text: textContent, t: Date.now()})"

# Save to file
obscura monitor https://example.com/status --interval 30 --save-to watch.jsonl

# Serve changes over HTTP (GET /last) + WebSocket (/events)
obscura monitor https://example.com --serve 127.0.0.1:9090
```

### Windows (PowerShell)

```powershell
# Fetch a page
.\obscura.exe fetch https://example.com --eval "document.title"

# Search DuckDuckGo
.\obscura.exe search "news iran" --engine duckduckgo --max-results 10

# Search with page scraping, save to file
.\obscura.exe search "rust" --depth page --scrape text --format ndjson -o C:\tmp\out.ndjson

# Monitor a page
.\obscura.exe monitor https://example.com --interval 30 --max-runs 5

# CDP server
.\obscura.exe serve --port 9222

# MCP server for AI agents
.\obscura.exe mcp
```

## Puppeteer / Playwright

### Puppeteer

```bash
npm install puppeteer-core
```

```javascript
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.connect({
  browserWSEndpoint: 'ws://127.0.0.1:9222/devtools/browser',
});

const page = await browser.newPage();
await page.goto('https://news.ycombinator.com');

const stories = await page.evaluate(() =>
  Array.from(document.querySelectorAll('.titleline > a'))
    .map(a => ({ title: a.textContent, url: a.href }))
);
console.log(stories);

await browser.disconnect();
```

### Playwright

```bash
npm install playwright-core
```

```javascript
import { chromium } from 'playwright-core';

const browser = await chromium.connectOverCDP({
  endpointURL: 'ws://127.0.0.1:9222',
});

const page = await browser.newContext().then(ctx => ctx.newPage());
await page.goto('https://en.wikipedia.org/wiki/Web_scraping');
console.log(await page.title());

await browser.close();
```

### Form submission & login

```javascript
await page.goto('https://quotes.toscrape.com/login');
await page.evaluate(() => {
  document.querySelector('#username').value = 'admin';
  document.querySelector('#password').value = 'admin';
  document.querySelector('form').submit();
});
// Obscura handles the POST, follows the 302 redirect, maintains cookies
```

## Benchmarks

Page load:

| Page | Obscura | Chrome |
|------|---------|--------|
| Static HTML | **51 ms** | ~500 ms |
| JS + XHR + fetch | **84 ms** | ~800 ms |
| Dynamic scripts | **78 ms** | ~700 ms |

The full benchmark suite (WPT conformance, obstacle course, real-world corpus, and vs-Chrome speed) lives in a separate repo: https://github.com/h4ckf0r0day/obscura-benchmark

## Stealth Mode

Enable with `--features stealth`.

### Anti-fingerprinting
- Per-session fingerprint randomization (GPU, screen, canvas, audio, battery)
- Realistic `navigator.userAgentData` (Chrome 145, high-entropy values)
- `event.isTrusted = true` for dispatched events
- Hidden internal properties (`Object.keys(window)` safe)
- Native function masking (`Function.prototype.toString()` → `[native code]`)
- `navigator.webdriver = undefined` (matches real Chrome)

### Tracker Blocking
- 3,520 domains blocked
- Blocks analytics, ads, telemetry, and fingerprinting scripts
- Prevents trackers from loading entirely
- Enabled automatically with `--stealth`

## CDP API

Obscura implements the Chrome DevTools Protocol for Puppeteer/Playwright compatibility.

| Domain | Methods |
|--------|---------|
| **Target** | createTarget, closeTarget, attachToTarget, createBrowserContext, disposeBrowserContext |
| **Page** | navigate, getFrameTree, addScriptToEvaluateOnNewDocument, lifecycleEvents |
| **Runtime** | evaluate, callFunctionOn, getProperties, addBinding |
| **DOM** | getDocument, querySelector, querySelectorAll, getOuterHTML, resolveNode |
| **Network** | enable, setCookies, getCookies, setExtraHTTPHeaders, setUserAgentOverride |
| **Fetch** | enable, continueRequest, fulfillRequest, failRequest (live interception), takeResponseBodyAsStream |
| **IO** | read, close (stream a large response body in chunks) |
| **Storage** | getCookies, setCookies, deleteCookies |
| **Input** | dispatchMouseEvent, dispatchKeyEvent |
| **LP** | getMarkdown (DOM-to-Markdown conversion) |

To download a large resource without one giant `Network.getResponseBody` blob, call `Fetch.takeResponseBodyAsStream` then read it in chunks with `IO.read` / `IO.close`. Response bodies over the cache limit (`OBSCURA_NETWORK_BODY_BUFFER_BYTES`, default 2 MiB) are not retained, so raise that limit when you intend to stream large downloads.
## CLI Reference

Every command is `obscura <command> [args]`. Run `obscura --help` for the command
list, or `obscura <command> --help` for a command's own flags and examples. The
sections below document **every** command and **every** argument in detail.

### Global flags

These are accepted before the subcommand (e.g. `obscura --proxy … fetch …`).
Flags marked *global* also work after the subcommand.

| Flag | Default | Scope | Description |
|------|---------|-------|-------------|
| `--proxy <URL>` | — | global | Route all traffic through an HTTP or SOCKS5 proxy, e.g. `http://127.0.0.1:8080` or `socks5://127.0.0.1:1080`. Inherited by `fetch`, `scrape` workers, `serve`, `search`, `monitor`, `warmup`, and `mcp`. |
| `--stealth` | off | global | Turn on stealth. Always applies a consistent browser fingerprint; with a `--features stealth` build it additionally enables full TLS/JA4 impersonation (Chrome 145) and tracker blocking. Applies to `fetch`, `serve`, `scrape`, `search`, `monitor`, and `mcp`. |
| `--allow-private-network` | off | global | Permit fetches to loopback, RFC1918 (192.168.x.x, 10.x.x.x), and link-local addresses. Blocked by default as an SSRF guard. Equivalent to `OBSCURA_ALLOW_PRIVATE_NETWORK=1` but per-process. Needed to hit `http://localhost:N` or LAN hosts. |
| `--user-agent <UA>` | Chrome 145 UA | — | Override the `User-Agent` string. |
| `--obey-robots` | off | — | Respect `robots.txt`. |
| `--storage-dir <DIR>` | — | — | Directory for persistent storage (cookies etc.) shared by the process. |
| `--v8-flags <FLAGS>` | — | — | Pass raw flags straight to V8 (same syntax as Chromium `--js-flags` / Node), applied once at startup. See **Tuning V8** below. |
| `--port <N>` | `9222` | — | Default port used by the bare `obscura` banner / implicit serve. Each server command has its own `--port`. |

### Environment variables

| Variable | Description |
|----------|-------------|
| `OBSCURA_SCRIPT_DEADLINE_MS` | Per-page script-execution budget in ms (default 30000). Raise for very heavy SPAs — see **Heavy SPAs** below. |
| `OBSCURA_ALLOW_PRIVATE_NETWORK` | `1` = same as `--allow-private-network`. |
| `OBSCURA_NETWORK_BODY_BUFFER_BYTES` | Max response-body bytes retained for `Network.getResponseBody` (default 2 MiB). Raise when streaming large downloads over CDP. |

### Tuning V8

Obscura embeds V8 directly. Use `--v8-flags` to pass raw flags through to V8, same syntax as Chromium's `--js-flags` and Node's command-line flags. Most common use is raising the heap cap to fix `JavaScript heap out of memory` on JS-heavy pages:

```bash
obscura --v8-flags "--max-old-space-size=4096" fetch <url>
obscura --v8-flags "--max-old-space-size=4096 --max-semi-space-size=64 --expose-gc" fetch <url>
```

### Heavy SPAs (script execution budget)

Obscura caps the page's script-execution phase so one slow or hung page cannot stall a worker. The default budget is 30s; pages that finish sooner return immediately, so the cap only affects pages that keep running. A very heavy React/Vue/Angular SPA on a slow network can need more time to boot before it fires its data requests. Raise the budget with `OBSCURA_SCRIPT_DEADLINE_MS` (milliseconds), and pair it with a matching navigation timeout in your CDP client:

```bash
OBSCURA_SCRIPT_DEADLINE_MS=60000 obscura serve --port 9222
```

---

### `obscura serve`

Start a Chrome DevTools Protocol server over WebSocket. Connect Puppeteer or
Playwright to `ws://HOST:PORT` and drive a real headless browser. See the
[Puppeteer / Playwright](#puppeteer--playwright) section for client code.

| Flag | Default | Description |
|------|---------|-------------|
| `--port <N>` | `9222` | WebSocket port to listen on. |
| `--host <ADDR>` | `127.0.0.1` | Bind address. Loopback-only by default; set `0.0.0.0` to accept connections from other hosts (e.g. inside Docker with `-p` mapping). |
| `--workers <N>` | `1` | Number of parallel worker processes, each an isolated browser, so multiple CDP sessions run concurrently. |
| `--allow-file-access` | off | Allow CDP clients to navigate to `file://` URLs. Off by default so a connection cannot read arbitrary local files; enable only for local testing on a trusted port. |
| `--proxy <URL>` | — | HTTP/SOCKS5 proxy for all served sessions. |
| `--user-agent <UA>` | — | Override the User-Agent for served sessions. |
| `--storage-dir <DIR>` | — | Persist cookies/storage across sessions in this directory. |
| `--stealth` | off | Enable anti-detection + tracker blocking (global flag). |
| `--quiet` | off | Suppress all logs. Useful for pages that flood the console with per-page script warnings. |

```bash
obscura serve --port 9222
obscura serve --port 9222 --stealth
obscura serve --host 0.0.0.0 --port 9222 --workers 4   # multi-session, Docker-reachable
```

---

### `obscura fetch <URL>`

Fetch and render **one** page (running its JavaScript), then either dump the
result in a chosen format or evaluate a JS expression with `--eval`. Pass
`--file` instead of a URL to run a **batch** of raw fetches concurrently.

| Argument / Flag | Default | Description |
|------|---------|-------------|
| `<URL>` | — | The page to fetch. Optional when `--file` is used. |
| `--dump <FMT>` | `html` | Output format: `html` (rendered DOM), `text` (visible text), `links` (all anchors), `markdown` (DOM→Markdown), `assets` (NDJSON of every sub-resource URL the page references), or `original` (raw response body, binary-safe — use for images/JSON/JS/CSS). |
| `--eval <EXPR>` / `-e` | — | JavaScript expression to evaluate; its value is printed. A bare `--eval` returns just that value; combined with `--dump`/`--selector` it runs the eval, lets async work settle, then reads the page. |
| `--selector <CSS>` | — | Wait for this CSS selector to appear before dumping. |
| `--wait-until <EVENT>` | `load` | Navigation wait condition: `load`, `domcontentloaded`, or `networkidle0`. |
| `--wait <SECS>` | `5` | Extra settle time (seconds) after the wait condition, for late async work. |
| `--timeout <SECS>` | `30` | Maximum navigation time in seconds (min 1). |
| `--output <FILE>` / `-o` | — | Write the dump/eval output to a file instead of stdout. |
| `--file <FILE>` | — | Read newline-delimited URLs (one per line; blank/`#` lines skipped; `-` = stdin) and batch-fetch them raw, printing one JSON status line per URL. For rendered/DOM batch output use `scrape`. |
| `--concurrency <N>` | `1` | URLs fetched concurrently in batch mode (ignored without `--file`). |
| `--user-agent <UA>` | — | Override the User-Agent for this fetch. |
| `--storage-dir <DIR>` | — | Persist cookies/storage in this directory. |
| `--quiet` / `-q` | off | Suppress the banner/logs. |
| `--stealth` | off | Anti-detection mode (global flag). |
| `--proxy <URL>` | — | Proxy for this fetch (global flag). |

```bash
obscura fetch https://example.com --eval "document.title"
obscura fetch https://example.com --dump links
obscura fetch https://news.ycombinator.com --dump html
obscura fetch https://example.com --dump text --output page.txt
obscura fetch https://picsum.photos/200/300 --dump original > photo.jpg
obscura fetch https://example.com --dump assets
obscura fetch https://example.com --wait-until networkidle0 --timeout 10
obscura fetch --file urls.txt --concurrency 8      # batch raw fetch
```

---

### `obscura scrape <URL...>`

Scrape **many** URLs in parallel using separate worker processes; run `--eval`
on each and print JSON or text. This is the rendered/DOM batch path at scale
(vs. `fetch --file`, which does raw fetches). Requires `obscura-worker` next to
the binary.

| Argument / Flag | Default | Description |
|------|---------|-------------|
| `<URL...>` | — | One or more URLs to scrape. |
| `--eval <EXPR>` / `-e` | — | JS expression evaluated on every page; its value is the record for that URL. |
| `--concurrency <N>` | `10` | Number of parallel worker processes. |
| `--format <FMT>` | `json` | Output: `json` or `text`. |
| `--timeout <SECS>` | `60` | Per-page timeout in seconds (min 1). |
| `--quiet` / `-q` | off | Suppress scrape progress on stderr (script-friendly output). |
| `--stealth` | off | Anti-detection mode (global flag). |
| `--proxy <URL>` | — | Proxy inherited by all workers (global flag). |

```bash
obscura scrape url1 url2 url3 --concurrency 25 \
  --eval "document.querySelector('h1').textContent" --format json
obscura scrape https://example.com --quiet --format json
obscura --proxy http://127.0.0.1:8080 scrape https://a.com https://b.com
```

---

### `obscura search <QUERY>`

Search a web engine and optionally scrape each result page. One shared core
powers this command, the MCP `octo_search` tool, and `octo-serve`. Results are
real browser navigations; add `--session` for a warm, higher-trust identity.

| Argument / Flag | Default | Description |
|------|---------|-------------|
| `<QUERY>` | — | The search query. |
| `--engine <ENGINE>` | `duckduckgo` | `google`, `bing`, `duckduckgo`, or `custom`. |
| `--engine-url <TEMPLATE>` | — | SERP URL template for `--engine custom`, with `{query}` and `{lang}` placeholders, e.g. `https://searx.be/search?q={query}`. |
| `--max-results <N>` | engine default | Ceiling on results. Obscura pages through the SERP (each page yields ~10) until this ceiling or the engine runs out. |
| `--depth <DEPTH>` | `serp` | `serp` = SERP links only; `page` = also open and scrape each result; `deep` = follow one level of same-host links from each result. |
| `--scrape <KIND>` | `text` for page/deep | What to extract from result pages: `none`, `text`, `html`, or `links`. |
| `--format <FMT>` | `json` | `json` (pretty object), `ndjson` (one result per line + a summary line), or `text` (rank/url/title rows). |
| `--output <FILE>` / `-o` | — | Write output to a file. |
| `--eval <EXPR>` | — | JS evaluated on every scraped result page; its value is attached to each result. |
| `--site <DOMAIN>` | — | Limit results to this domain (repeatable). |
| `--exclude-site <DOMAIN>` | — | Exclude this domain (repeatable). |
| `--site-exact` | off | Match `--site` exactly, without subdomains. |
| `--lang <LANG>` | `en` | Search language / region hint. |
| `--fallback <ENGINE>` | — | Engine to retry once with when the primary returns zero results (e.g. blocked). |
| `--concurrency <N>` | `10` | Parallel result-page scrapes (for `--depth page`/`deep`). |
| `--timeout <SECS>` | `30` | Per-page navigation timeout. |
| `--wait <SECS>` | — | Extra settle time per result page. |
| `--session <DIR>` | — | Persist/reuse the cookie jar here (warm returning-visitor session — see **Warm sessions**). |
| `--stealth` | off | Anti-detection mode (global flag). |
| `--proxy <URL>` | — | Proxy (global flag); use a residential proxy for Google. |
| `--allow-private-network` | off | Permit private/loopback targets (global flag). |

When an engine serves a wall instead of results, the error names the **captcha
type** (reCAPTCHA v2/v3, hCaptcha, Google "unusual traffic", Cloudflare challenge,
consent wall) and the **likely reason** it triggered (IP reputation, low
behavioral score, request velocity, regional consent), with how to recover.

```bash
obscura search "news iran" --engine duckduckgo --max-results 20
obscura search "rust tokio" --depth page --scrape text --format ndjson -o results.ndjson
obscura search "climate report" --site nature.com --site nasa.gov
obscura search "news" --engine google --fallback duckduckgo
obscura search "q" --engine custom --engine-url "https://searx.be/search?q={query}"
obscura search "rust jobs" --engine bing --session ~/.obscura/session
```

---

### `obscura warmup`

Mature a **reusable session** by really browsing an engine for a set duration:
Obscura runs genuine SERP queries, opens a couple of results each, optionally
visits your own target URLs, pauses naturally, and saves the accumulated cookie
jar. Nothing is fabricated — it is a real returning-visitor history, a legitimate
signal that tends to raise anti-bot scores. Live progress is printed to stderr.

| Flag | Default | Description |
|------|---------|-------------|
| `--session <DIR>` | — (**required**) | Directory to save the session (`cookies.json`) into; created if missing. |
| `--engine <ENGINE>` | `bing` | Engine to browse: `google`, `bing`, or `duckduckgo`. |
| `--minutes <N>` | `15` | How long to browse (accepts fractions, e.g. `0.5`). |
| `--query <TEXT>` | built-in generic set | Seed query to browse (repeatable). Overrides the default set. |
| `--url <URL>` | — | Target URL to visit directly, so that site accumulates its own real cookies too (repeatable). Interleaved with the search sessions. |
| `--proxy <URL>` | — | Proxy to browse through (global flag). |
| `--stealth` | off | Anti-detection mode (global flag). |

```bash
obscura warmup --engine bing --minutes 15 --session ~/.obscura/session
obscura warmup --engine duckduckgo --minutes 5 --session ./sess --query "rust jobs"
obscura warmup --minutes 10 --session ./sess --url https://example.com --url https://news.ycombinator.com
```

---

### `obscura session-info`

Print a human-readable summary of a saved `--session` so you don't have to open
`cookies.json`: total cookie count, number of domains, a breakdown of
persistent/session/secure/expired cookies, when it was last updated, and the top
domains by cookie count.

| Flag | Default | Description |
|------|---------|-------------|
| `--session <DIR>` | — (**required**) | The session directory (must contain `cookies.json`). |
| `--top <N>` | `15` | How many top domains to list. |

```bash
obscura session-info --session ~/.obscura/session
obscura session-info --session ./sess --top 30
```

```
session: ~/.obscura/session
last updated: 2m ago
cookies: 103 total (30 domains) — 67 persistent, 36 session, 79 secure, 1 expired
top domains:
   16  weather.com
   10  bing.com
   ...
```

---

### `obscura monitor <URL>`

Watch a page and emit a record whenever a watched value changes. Values are
hashed and de-duplicated, so only real changes are emitted. Prints NDJSON by
default, or serves the stream over HTTP + WebSocket with `--serve`.

`--condition` and `--on-change` are JavaScript run with the watched element in
scope (so bare `textContent` works).

| Argument / Flag | Default | Description |
|------|---------|-------------|
| `<URL>` | — | The page to watch. |
| `--selector <CSS>` | whole document | CSS selector of the element to watch. |
| `--condition <JS>` | always true | JS predicate; a truthy result marks a candidate change. |
| `--on-change <JS>` | element text | JS producing the value to capture and emit. |
| `--interval <SECS>` | `60` | Polling interval in seconds. |
| `--max-runs <N>` | `0` (forever) | Stop after N polls. |
| `--min-change-interval <SECS>` | — | Suppress emitting more than one change per this many seconds (debounce). |
| `--timeout <SECS>` | — | Per-poll navigation timeout. |
| `--wait <SECS>` | — | Extra settle time per poll. |
| `--save-to <FILE>` | — | Append each change as an NDJSON line to this file. |
| `--serve <HOST:PORT>` | — | Serve changes over HTTP (`GET /last`, `/health`) + WebSocket (`/events`) instead of printing. |
| `--ws-port <N>` | HTTP port + 1 | WebSocket port when using `--serve`. |
| `--token <TOKEN>` | — | Bearer token required when `--serve` binds a non-loopback host. |
| `--session <DIR>` | — | Persist/reuse the cookie jar in this directory. |
| `--stealth` | off | Anti-detection mode (global flag). |
| `--proxy <URL>` | — | Proxy (global flag). |

```bash
obscura monitor https://example.com/status
obscura monitor https://x.com/page --selector "article:first-child" \
    --condition "textContent.includes('2026')" \
    --on-change "JSON.stringify({text: textContent, t: Date.now()})"
obscura monitor https://example.com/status --interval 30 --save-to watch.jsonl
obscura monitor https://example.com --serve 127.0.0.1:9090   # GET /last, WS /events
```

---

### `obscura octo-serve`

Expose the `search` core over HTTP and WebSocket, so other services can query it.
`POST /search` accepts a JSON body (same fields as the `search` flags) and returns
a `SearchResponse`; send `Accept: application/x-ndjson` or `"format":"ndjson"` for
a streamed NDJSON response. `GET /health` is a liveness check. A WebSocket endpoint
streams `result` frames then a `summary` frame.

| Flag | Default | Description |
|------|---------|-------------|
| `--host <ADDR>` | `127.0.0.1` | Bind address. |
| `--port <N>` | `8080` | HTTP/WS port. |
| `--ws-port <N>` | HTTP port + 1 | Separate WebSocket port. |
| `--token <TOKEN>` | — | Bearer token; required to bind a non-loopback host. |
| `--session <DIR>` | — | Persist/reuse the cookie jar in this directory. |
| `--stealth` | off | Anti-detection mode (global flag). |
| `--proxy <URL>` | — | Proxy (global flag). |

```bash
obscura octo-serve --port 8080
curl -s localhost:8080/search -d '{"query":"rust tokio","max_results":5}'
curl -s localhost:8080/search -H 'Accept: application/x-ndjson' \
  -d '{"query":"rust","depth":"page","scrape":"text"}'
```

---

### `obscura mcp`

Run the Model Context Protocol server (see the [MCP section](#mcp-model-context-protocol) below for tools and client config).

| Flag | Default | Description |
|------|---------|-------------|
| `--http` | off | Serve over HTTP instead of stdio. Endpoint: `http://HOST:PORT/mcp`. |
| `--host <ADDR>` | `127.0.0.1` | Bind address (HTTP transport). |
| `--port <N>` | `3000` | Port (HTTP transport). |
| `--user-agent <UA>` | — | Custom User-Agent for browser tools. |
| `--proxy <URL>` | — | HTTP/SOCKS5 proxy. |
| `--stealth` | off | Anti-detection mode (global flag). |

## MCP (Model Context Protocol)

Obscura ships an MCP server that exposes browser automation tools to AI agents (Claude Desktop, Cursor, etc.).

### Start

**stdio** (default) — for Claude Desktop and MCP clients that launch a subprocess:

```bash
obscura mcp
```

**HTTP** — for clients that connect over the network:

```bash
obscura mcp --http --port 8080
# endpoint: http://127.0.0.1:8080/mcp
```

Optional flags (both transports):

| Flag | Description |
|------|-------------|
| `--proxy <URL>` | HTTP/SOCKS5 proxy |
| `--user-agent <UA>` | Custom User-Agent string |
| `--stealth` | Enable anti-detection mode |

### Claude Desktop config

```json
{
  "mcpServers": {
    "obscura": {
      "command": "obscura",
      "args": ["mcp"]
    }
  }
}
```

### Tools

**Navigation & page state**

| Tool | Description |
|------|-------------|
| `browser_navigate` | Navigate to a URL (`url`, optional `waitUntil`: `load` / `domcontentloaded` / `networkidle0`). |
| `browser_back` / `browser_forward` | Go back / forward in page history. |
| `browser_reload` | Reload the current page. |
| `browser_snapshot` | Return the current page URL, title, and body text. |
| `browser_markdown` | Extract the page as Markdown (headings, lists, links, code) — token-dense structured content. |
| `browser_links` | List every anchor as `{text, href}` (one JSON object per line). |
| `browser_search` | Find substring matches in the visible page text, with surrounding context. |
| `browser_wait_for` | Wait for a CSS selector to appear (`selector`, optional `timeout` in seconds). |
| `browser_wait_for_text` | Wait until a substring appears anywhere in the rendered text. |

**Interaction**

| Tool | Description |
|------|-------------|
| `browser_interactive_elements` | List clickable/typeable elements with stable `ref` IDs (e.g. `e3`) — use before clicking/filling to refer to elements by ref. |
| `browser_click` | Click an element by CSS selector or ref. |
| `browser_fill` | Set an input value (fires `input` + `change`). |
| `browser_type` | Append text to an input. |
| `browser_press_key` | Dispatch a keyboard event (`key`, optional `selector`). |
| `browser_select_option` | Select an `<option>` by value or text. |
| `browser_scroll` | Scroll page or element (`direction`, `amount`); use `bottom` to trigger infinite-scroll. |
| `browser_detect_forms` | List every `<form>` with action, method, and its inputs. |
| `browser_fill_form` | Fill multiple inputs in one call (array of `{ref?, selector?, value, type?}`; `type` = text/check/uncheck/select). |

**Extraction & inspection**

| Tool | Description |
|------|-------------|
| `browser_evaluate` | Evaluate a JavaScript expression and return the result. |
| `browser_extract` | Extract a structured object from a `{field: css_selector}` map; `selector@attr` reads an attribute, `field[]` returns an array. |
| `browser_get_attribute` | Read an element's attribute (href, src, value, class, data-*, …). |
| `browser_count` | Count elements matching a CSS selector (cheap existence/pagination probe). |
| `browser_network_requests` | List network requests made by the current page. |
| `browser_console_messages` | Return console messages logged by the page. |

**Tabs**

| Tool | Description |
|------|-------------|
| `browser_tab_new` | Open a new isolated tab; returns its ID. |
| `browser_tab_list` | List all open tabs (ID, URL, title, active). |
| `browser_tab_switch` | Switch the active tab. |
| `browser_tab_close` | Close a tab by ID (default: active). |

**Session, cookies & search**

| Tool | Description |
|------|-------------|
| `browser_get_cookies` | Return all cookies in the jar (one JSON object per line). |
| `browser_set_cookie` | Add or replace a cookie (skip a login when you already have a token). |
| `browser_clear_cookies` | Wipe every cookie from the jar. |
| `browser_storage_state` | Export cookies + localStorage + sessionStorage as JSON (save an authenticated session). |
| `browser_set_storage_state` | Restore state previously exported by `browser_storage_state`. |
| `octo_search` | Web search over an engine (`query`, `engine`, `max_results`, `lang`, `site`, `depth`, `scrape`, `fallback`, …) — the same core as the `search` command; returns a full `SearchResponse`. |
| `browser_close` | Close the page and reset browser state. |

## Integrations

- **[Hermes agent plugin](https://github.com/SGavrl/hermes-plugin-obscura)**: run [Hermes](https://github.com/NousResearch/hermes-agent) agent browser tasks on Obscura. The plugin spawns `obscura serve` per session (or connects to an already running server) and drives it over CDP, with optional `--stealth`.

## License

Apache 2.0

---
