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

# Then reuse it anywhere
obscura search "..." --engine bing --session ~/.obscura/session
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

### Tuning V8

Obscura embeds V8 directly. Use `--v8-flags` to pass raw flags through to V8, same syntax as Chromium's `--js-flags` and Node's command-line flags. Most common use is raising the heap cap to fix `JavaScript heap out of memory` on JS-heavy pages:

```bash
obscura --v8-flags "--max-old-space-size=4096" fetch <url>
```

### Heavy SPAs (script execution budget)

Obscura caps the page's script-execution phase so one slow or hung page cannot stall a worker. The default budget is 30s; pages that finish sooner return immediately, so the cap only affects pages that keep running. A very heavy React/Vue/Angular SPA on a slow network can need more time to boot before it fires its data requests. Raise the budget with `OBSCURA_SCRIPT_DEADLINE_MS` (milliseconds), and pair it with a matching navigation timeout in your CDP client:

```bash
OBSCURA_SCRIPT_DEADLINE_MS=60000 obscura serve --port 9222
```

### `obscura serve`

Start a CDP WebSocket server.

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | `9222` | WebSocket port |
| `--proxy` | — | HTTP/SOCKS5 proxy URL |
| `--stealth` | off | Enable anti-detection + tracker blocking |
| `--workers` | `1` | Number of parallel worker processes |
| `--obey-robots` | off | Respect robots.txt |

### `obscura fetch <URL>`

Fetch and render a single page.

| Flag | Default | Description |
|------|---------|-------------|
| `--dump` | `html` | Output: `html`, `text`, `links`, `markdown`, `assets` (NDJSON of every sub-resource URL the page references), or `original` (raw response body) |
| `--eval` | — | JavaScript expression to evaluate |
| `--wait-until` | `load` | Wait: `load`, `domcontentloaded`, `networkidle0` |
| `--timeout` | `30` | Maximum navigation time in seconds |
| `--selector` | — | Wait for CSS selector |
| `--stealth` | off | Anti-detection mode |
| `--output` | — | Write dump or eval output to a file |
| `--quiet` | off | Suppress banner |
| `--proxy` | — | Inherited global HTTP/SOCKS5 proxy URL |

### `obscura scrape <URL...>`

Scrape multiple URLs in parallel with worker processes.

| Flag | Default | Description |
|------|---------|-------------|
| `--concurrency` | `10` | Parallel workers |
| `--eval` | — | JS expression per page |
| `--format` | `json` | Output: `json` or `text` |
| `--quiet` | off | Suppress scrape progress on stderr |
| `--proxy` | — | Inherited global HTTP/SOCKS5 proxy URL for all workers |

### `obscura search <QUERY>`

Search a web engine and optionally scrape the results.

| Flag | Default | Description |
|------|---------|-------------|
| `--engine` | `duckduckgo` | `google`, `bing`, `duckduckgo`, `custom` |
| `--max-results` | — | Ceiling on results |
| `--depth` | `serp` | `serp`, `page` (scrape each result), `deep` (follow same-host links) |
| `--scrape` | — | `none`, `text`, `html`, `links` (default `text` for `page`/`deep`) |
| `--format` | `json` | `json`, `ndjson`, `text` |
| `--output` | — | Write to file |
| `--eval` | — | JS evaluated on every scraped result page |
| `--site` | — | Limit to domain (repeatable) |
| `--exclude-site` | — | Exclude domain (repeatable) |
| `--lang` | `en` | Search language |
| `--fallback` | — | Engine to retry when primary returns zero results |
| `--proxy` | — | HTTP/SOCKS5 proxy URL |
| `--stealth` | off | Anti-detection mode |
| `--concurrency` | `10` | Parallel page scrapes |
| `--timeout` | `30` | Per-page timeout in seconds |
| `--session` | — | Persist/reuse the browser cookie jar in this directory |

### `obscura warmup`

Mature a reusable browser session by really browsing an engine for a while.

| Flag | Default | Description |
|------|---------|-------------|
| `--engine` | `bing` | `google`, `bing`, `duckduckgo` |
| `--minutes` | `15` | How long to browse |
| `--session` | — (required) | Directory to save the session (cookies) into |
| `--query` | generic set | Seed query to browse (repeatable) |
| `--proxy` | — | HTTP/SOCKS5 proxy URL |

### `obscura monitor <URL>`

Watch a page and emit changes as NDJSON (or serve over HTTP/WS).

| Flag | Default | Description |
|------|---------|-------------|
| `--selector` | whole document | CSS selector to watch |
| `--condition` | always true | JS predicate; truthy marks a candidate change |
| `--on-change` | element text | JS producing the value to capture |
| `--interval` | `60` | Polling interval in seconds |
| `--max-runs` | `0` (forever) | Stop after N polls |
| `--min-change-interval` | — | Min seconds between emissions |
| `--save-to` | — | Append each change as NDJSON to this file |
| `--serve` | — | Serve changes over HTTP+WS at `host:port` |
| `--session` | — | Persist/reuse the browser cookie jar in this directory |
| `--proxy` | — | HTTP/SOCKS5 proxy URL |
| `--stealth` | off | Anti-detection mode |

### `obscura octo-serve`

Expose search over HTTP and WebSocket.

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | `8080` | HTTP/WS port |
| `--token` | — | Bearer token required for non-loopback binds |
| `--session` | — | Persist/reuse the browser cookie jar in this directory |
| `--proxy` | — | HTTP/SOCKS5 proxy URL |
| `--stealth` | off | Anti-detection mode |

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

| Tool | Description |
|------|-------------|
| `browser_navigate` | Navigate to a URL (`url`, optional `waitUntil`: `load` / `domcontentloaded` / `networkidle0`) |
| `browser_snapshot` | Return the current page URL, title, and body text |
| `browser_click` | Click an element by CSS selector |
| `browser_fill` | Set an input value (triggers `input` + `change` events) |
| `browser_type` | Append text to an input |
| `browser_press_key` | Dispatch a keyboard event (`key`, optional `selector`) |
| `browser_select_option` | Select an `<option>` by value or text |
| `browser_evaluate` | Evaluate a JavaScript expression and return the result |
| `browser_wait_for` | Wait for a CSS selector to appear (`selector`, optional `timeout` in seconds) |
| `browser_network_requests` | List network requests made by the current page |
| `browser_console_messages` | Return console messages logged by the page |
| `browser_close` | Close the page and reset browser state |

## Integrations

- **[Hermes agent plugin](https://github.com/SGavrl/hermes-plugin-obscura)**: run [Hermes](https://github.com/NousResearch/hermes-agent) agent browser tasks on Obscura. The plugin spawns `obscura serve` per session (or connects to an already running server) and drives it over CDP, with optional `--stealth`.

## License

Apache 2.0

---
