# Obscura Octo — nuove feature (`search`, `monitor`)

Branch di lavoro: **`octo`**. Questo file e' la fonte di verita' del piano.
Nessun codice ancora scritto: qui c'e' *come* lavorare e *cosa* costruire.

> Nota: `extract` NON e' piu' nel piano. Esiste gia' come tool MCP
> `browser_extract` (`crates/obscura-mcp/src/lib.rs`): schema `campo -> selettore`
> con sintassi `selector@attr` e suffisso `field[]` per le liste, estrae dalla
> pagina corrente. Riscriverlo sarebbe duplicazione. Se serve estrazione dentro
> `search`/`monitor`, si RIUSA quella logica, non se ne crea un comando nuovo.

---

## 0. Regole di lavoro (leggere prima di toccare qualsiasi cosa)

### 0.1 `main` non si tocca — mai

`main` e' il **fork upstream di Obscura** (`h4ckf0r0day/obscura`). Serve a poter
riallineare (`git merge`/`rebase` da upstream) quando loro rilasciano fix. Se
sporchiamo `main` con roba nostra, ogni update upstream diventa un conflitto.

Regole ferree:

1. Tutto il lavoro nostro vive su `octo` (o feature-branch che partono da `octo`).
2. `main` resta un mirror pulito di upstream. Aggiornamento upstream:
   ```
   git checkout main
   git pull upstream main          # solo fast-forward, nessun commit nostro
   git checkout octo
   git rebase main                 # riporta le nostre feature sopra l'upstream
   ```
3. Le nostre modifiche devono restare **rebasabili**: piccole, isolate, con la
   minima superficie di contatto sui file upstream (vedi 0.2). Meno righe tocchiamo
   nei file di upstream, meno conflitti al prossimo merge.

### 0.2 Isolamento del codice: nuovo crate, non patch sparse

Per non spargere codice nostro dentro i file upstream (che poi confligge), la
logica delle feature va in **un crate nuovo e nostro**:

```
crates/obscura-octo/          # <-- tutto il nostro codice vive qui
  src/
    lib.rs                    # API pubblica: run_search / run_monitor
    search.rs
    monitor.rs
    schema.rs                 # tipi condivisi (SearchRequest, SearchResult, ...)
    security.rs               # guardie di rete condivise (vedi sez. 4)
    output.rs                 # sink di output per-superficie (file / stdout / HTTP / WS)
    config.rs                 # default ottimali + merge con override (vedi sez. 5)
    surface.rs                # adattatori CLI / MCP / HTTP / WS su un core unico
```

I punti di contatto con i file upstream devono essere **minimi e additivi**:

- `crates/obscura-cli/src/main.rs`: solo le varianti nuove in `enum Command` +
  i rami `match` che chiamano `obscura_octo::…`. Nessuna logica inline.
- `crates/obscura-mcp/src/lib.rs`: solo le entry in `tools/list` + i rami di
  dispatch, che delegano a `obscura_octo`.
- `Cargo.toml` workspace: aggiunta del membro `obscura-octo`.

Cosi un merge upstream tocca al massimo poche righe additive, facili da risolvere.

### 0.3 Un solo core, quattro superfici (requisito: CLI + MCP + HTTP + WS)

Ogni feature si usa da **CLI, MCP, HTTP e WS**. Per non riscrivere la logica 4
volte (e vederla divergere), si implementa **una funzione core** per feature che
prende un input tipizzato e ritorna un output tipizzato/stream. Le superfici sono
adattatori sottili sopra il core:

```
                 ┌─────────────────────────────┐
   CLI  args ───►│                             │
   MCP  json ───►│  core: run_search /          │──► sink di output (sez. 6)
   HTTP body ──►│        run_monitor            │
   WS   msg  ───►│                             │
                 └─────────────────────────────┘
```

- **Input**: una struct `…Request` (serde `Deserialize`) — la stessa per HTTP body,
  MCP `arguments`, e WS message; la CLI la costruisce dai flag.
- **Output**: NON e' hardcoded nel core. Il core emette **eventi/record** verso un
  `OutputSink` (trait) scelto dalla superficie: file/stdout per la CLI, corpo JSON
  per HTTP one-shot, frame WS per lo stream (vedi sez. 6).
- Le superfici NON contengono business logic: fanno parsing + costruiscono la
  Request + scelgono il sink + chiamano il core.

Le nuove rotte HTTP/WS vivono in un piccolo server nostro dentro `obscura-octo`;
NON estendiamo il server CDP di upstream.

### 0.4 Riuso, non reinvenzione

Prima di scrivere logica nuova, riusare cio' che esiste:

- Navigazione + render: `obscura_browser::Page` (`navigate_with_wait`, `settle`,
  `evaluate`, `evaluate_with_timeout`, `with_dom`).
- **Estrazione dati da una pagina: gia' fatta** — tool MCP `browser_extract`
  (schema `field -> selector`, `selector@attr`, `field[]`). Quando `search`
  (depth page/deep) deve estrarre campi da una pagina risultato, RIUSA quella
  logica (spostandola in `obscura-octo`/`obscura-browser` in modo condiviso se
  serve chiamarla anche fuori dall'MCP), non ne crea un'altra.
- Text/markdown/links/assets: helper gia' in `obscura-cli/src/main.rs`
  (`extract_readable_text`, `dump_markdown`, `dump_links`). Se servono anche fuori
  dalla CLI, spostarli in `obscura-browser` in un commit separato e additivo.

### 0.5 Vincoli di build/test (da AGENTS.md — non negoziabili)

1. `cargo build --release` (o `-p obscura-octo` / `-p obscura-cli` mentre si itera)
   deve compilare pulito.
2. Test con **`cargo nextest run`**, MAI `cargo test` (un solo isolate V8 per
   processo; `nextest` isola ogni test in un processo).
3. **Obstacle course 33/33** deve restare verde (repo `obscura-benchmark`):
   ```
   OBSCURA_BIN=./target/release/obscura python3 obstacle-course/run.py --runs 1 --warmup 0
   ```
4. Niente `cargo fmt` globale (il tree non e' rustfmt-clean): formattare a mano
   come i file intorno.
5. Ops panic-safe, watchdog e hard-deadline vanno rispettati (sez. 4).
6. Commit/PR/commenti: brevi, fattuali, niente em dash, niente filler AI.

### 0.6 Definition of Done (per ogni feature)

- [ ] Core in `obscura-octo` con test unit (`nextest`) su fixture offline.
- [ ] Superfici CLI + MCP + HTTP + WS che delegano al core, con parsing testato.
- [ ] Sink di output corretto per ogni superficie (sez. 6).
- [ ] Default ottimali applicati + override per-superficie testati (sez. 5).
- [ ] Guardie di sicurezza di rete applicate (sez. 4) e testate.
- [ ] `--help` CLI e `tools/list` MCP aggiornati e coerenti.
- [ ] Build release pulita, `nextest` verde, obstacle course 33/33.
- [ ] Doc aggiornata (`docs/` + README se serve) in commit separato.

---

## Stato implementazione

- **`search`**: IMPLEMENTATO nel crate `crates/obscura-octo` (core condiviso +
  superfici CLI `obscura search`, HTTP/WS `obscura octo-serve`, MCP `octo_search`).
  Test offline in `crates/obscura-octo/tests` (core+engine, HTTP, WS) verdi.
  Markdown come `--scrape` non incluso (richiede V8); disponibile via `--eval`.
  Paginazione: GET-offset per Google/Bing/custom (`{offset}`), e POST del
  next-form per DuckDuckGo (con Referer della pagina precedente + cookie jar
  condiviso, altrimenti DDG rende una pagina vuota). `--max-results` e' un tetto:
  il numero reale dipende da quanti risultati distinti da' il motore. `--depth
  serp` ritorna solo i link SERP; per il contenuto serve `--depth page`.
  Motori: **DuckDuckGo** funziona senza stealth (default). **Bing** funziona col
  build stealth (`--features stealth`, eseguito con `--stealth`): i suoi link sono
  redirect `bing.com/ck/a?...&u=a1<base64url>` che decodifichiamo. **Google**
  blocca via reCAPTCHA su reputazione IP anche con stealth: serve un `--proxy`
  residenziale. Se un motore serve una pagina anti-bot il core ritorna un `error`
  esplicito; `--fallback duckduckgo` ripiega in automatico. Output su file con
  `-o/--output <path>` (json/ndjson/text).
  Build stealth su Windows: `scripts/build-stealth.bat` (richiede MSVC Build
  Tools + NASM + LLVM/libclang + Ninja; vedi lo script).
- **`monitor`**: da fare.

---

## 1. `search` — cerca con Obscura e scrapperizza

Obscura e' un browser: cercare = aprire il motore, renderizzare la SERP (JS
compreso), estrarre i risultati dal DOM, opzionalmente scrapperizzare ogni link.

### 1.1 CLI (i flag sovrascrivono i default ottimali — sez. 5)

```
obscura search "ultime news su guerra iran" \
  --engine google            # google | bing | duckduckgo
  --max-results 20           # quanti risultati (cap rigido)
  --lang it                  # lingua risultati
  --site example.com         # LIMITA i risultati a uno o piu' domini (ripetibile)
  --exclude-site pinterest.com  # esclude domini (ripetibile)
  --depth serp               # serp | page | deep
  --scrape text              # cosa salvare per pagina: text | markdown | html | links | none
  --format json              # json | ndjson | text
  --output risultati.json    # CLI: salva su file (default stdout)
  --eval "document.title"    # JS valutato su ogni pagina (depth page/deep)
  --wait 3                   # secondi di settle dopo il load
  --concurrency 5            # pagine risultato in parallelo (depth page/deep)
  --timeout 20               # timeout per navigazione
```

### 1.2 `--site` / limit site (miglioramento richiesto)

Limitare la ricerca a dominio/i:

- Implementazione **doppia**, per robustezza:
  1. inietta l'operatore nel motore (`site:example.com` nella query) — riduce il
     rumore a monte;
  2. **filtra comunque i risultati** lato nostro sull'host (allow-list `--site`,
     deny-list `--exclude-site`), perche' i motori non sempre onorano l'operatore.
- `--site` ripetibile (piu' domini in OR). Match per host canonico + sottodomini
  (`example.com` include `www.example.com`), configurabile con `--site-exact`.
- Utile anche per sicurezza: con `--site` la depth `deep` non esce dai domini
  consentiti.

### 1.3 Engine (astratto, non hardcoded)

Trait `SearchEngine` con `build_url(query, lang, site, page)` e
`parse_serp(dom) -> Vec<SearchResult>`. Aggiungere un motore = una impl.

| Engine | URL | Estrazione SERP |
|---|---|---|
| `duckduckgo` (html) | `https://html.duckduckgo.com/html/?q=Q` | `a.result__a` — piu' stabile, spesso senza JS. **Default consigliato** |
| `google` | `https://www.google.com/search?q=Q&hl=LANG` | `div.g a`, oppure decodifica `a[href^="/url?q="]` |
| `bing` | `https://www.bing.com/search?q=Q&cc=LANG` | `ol#b_results h2 a` |

Robustezza: selettori SERP come **dati con fallback** in ordine (cambiano
spesso); se tutti falliscono → errore chiaro, mai panic. `--stealth` e `--proxy`
globali rispettati. `--fallback bing` se l'engine primario ritorna 0 risultati.

### 1.4 Depth + scrape

- `serp`: solo risultati (rank, title, url, snippet).
- `page`: SERP + naviga ogni risultato e ne salva il contenuto secondo `--scrape`
  (text/markdown/html/links) + eventuale `--eval`.
- `deep`: come `page` + segue link interni allo stesso dominio (o ai `--site`),
  1 livello, con budget rigido = `--max-results`.

`--scrape none` = solo metadati SERP anche con depth page (utile per mappare
rapidamente). Dedup per URL canonico; scarta URL non http(s) e link interni al
motore.

### 1.5 Output tipizzato (contenuto uguale su tutte le superfici; il *dove* cambia — sez. 6)

```jsonc
{
  "query": "...", "engine": "duckduckgo", "lang": "it", "sites": ["example.com"],
  "results": [
    { "rank": 1, "title": "...", "url": "https://...", "snippet": "...",
      "scrape": { "text": "...", "markdown": null, "links": [...] },  // depth page/deep
      "eval": <valore> }
  ],
  "took_ms": 1234
}
```

Con `--format ndjson` (o su stream WS) ogni risultato e' emesso **appena pronto**
come una riga/frame, cosi la CLI su file e lo stream WS ricevono i risultati in
tempo reale invece di aspettare il batch completo.

---

## 2. `monitor` — watch continuo con streaming

Osserva una pagina e segnala i cambiamenti in tempo reale.

### 2.1 CLI

```
obscura monitor https://example.com/status \
  --selector "article:first-child" \
  --interval 60 \
  --condition "textContent.includes('2026')" \   # JS truthy = candidato cambiamento
  --on-change "JSON.stringify({text:textContent, t:Date.now()})" \
  --save-to watch.jsonl \        # CLI: append NDJSON su file
  --serve 127.0.0.1:9090 \       # HTTP GET (ultimo valore) + WS (broadcast) su una porta
  --token <SEGRETO> \            # richiesto se il bind non e' loopback (sez. 4.4)
  --max-runs 0 \                 # 0 = infinito
  --timeout 20                   # timeout per ciclo di navigazione
```

### 2.2 Streaming (HTTP + WS su una porta sola)

Server tokio in `obscura-octo`:
- `GET /last` → ultimo valore JSON (HTTP).
- `GET /events` (WS) → broadcast di ogni cambiamento.
- `GET /health` → stato (run count, ultimo errore, uptime).
- File: append NDJSON, una riga per cambiamento.

MCP: `octo_monitor_start` / `_status` / `_stop` (avvio in background + stato in
pull; il push resta su WS/HTTP, l'MCP stdio non ha canale push).

### 2.3 Core + stato

`run_monitor(MonitorRequest)`:
1. (opz.) avvia il server HTTP/WS su task dedicato.
2. Loop: naviga (timeout per ciclo + watchdog) → valuta `condition` → se truthy
   valuta `on-change` → confronta hash col valore precedente → se diverso: salva
   NDJSON, broadcast WS, aggiorna "last" → `sleep(interval)`.
`MonitorState`: ultimo valore + hash, broadcast channel, contatori, ultimo errore.

### 2.4 Robustezza

- Timeout per ciclo; fallimento navigazione → log + retry + **backoff** su errori
  ripetuti.
- `--max-runs N`; `0` = infinito.
- **Debounce** (`--min-change-interval`) contro lo sfarfallio.
- **Backpressure WS** bounded: un client lento non blocca il monitor.
- Payload sempre `{value, hash, run, ts}` → consumo idempotente lato client.

---

## 3. Piano di implementazione (ordine)

1. **Scheletro crate** `obscura-octo` + membro workspace `Cargo.toml`.
   `lib.rs` con firme `run_search/run_monitor` + tipi in `schema.rs` (stub, test
   placeholder).
2. **`config.rs`** (default + merge override, sez. 5) e **`output.rs`** (sink
   per-superficie, sez. 6): testati per primi, offline.
3. **`security.rs`**: guardie condivise (sez. 4), testate offline.
4. **`search`**: trait engine + parser SERP + `--site` + depth/scrape +
   concorrenza + emissione incrementale. Test su SERP fixture salvate.
5. **`monitor`**: loop + server HTTP/WS + stato. Test su fixture che cambia.
6. **Superfici finali**: comandi CLI, tool MCP, rotte HTTP/WS; `--help`,
   `tools/list`, `docs/`.
7. **Gate**: build release pulita, `nextest` verde, obstacle course 33/33.

---

## 4. Sicurezza di rete (vale per tutte le feature)

### 4.1 SSRF — riuso del gate esistente
Fetch/navigazioni verso loopback / RFC1918 / link-local **bloccate di default**
(`validate_fetch_url`, env `OBSCURA_ALLOW_PRIVATE_NETWORK`, flag
`--allow-private-network`). Nessun bypass. `search --depth deep` ri-valida ogni
URL seguito e ogni redirect; con `--site` non esce dai domini consentiti.

### 4.2 Limiti di risorse
Timeout per navigazione + **hard-deadline di processo** + watchdog V8. Cap su
body (`OBSCURA_NETWORK_BODY_BUFFER_BYTES`), su risultati/pagine (`--max-results`),
concorrenza a semaforo. Rotte HTTP/WS con `MAX_BODY_BYTES` come l'MCP HTTP.

### 4.3 Nessuna injection nei snippet JS
Selettori, `--site`, `condition`, `on-change`, `eval`, valori: **sempre** passati
a JS come dati (`serde_json::to_string`), mai per concatenazione. JS utente sotto
`evaluate_with_timeout` + watchdog.

### 4.4 Server `monitor` — bind e auth
Bind **loopback di default**. Bind pubblico → **`--token` obbligatorio**
(`Authorization: Bearer`; mai token in query string su bind pubblico). **Origin
allowlist** (pattern `OBSCURA_MCP_ALLOWED_ORIGINS`). CORS permissivo solo senza
allowlist. Rate-limit minimale.

### 4.5 Output e privacy
Mai token/credenziali in NDJSON o URL. Proxy (`--proxy`, puo' contenere
credenziali) passato via env `OBSCURA_PROXY`, non su argv.

---

## 5. Default ottimali + override per-superficie (requisito richiesto)

Ogni parametro ha un **default sensato** in `config.rs`, cosi lo strumento
funziona bene "a scatola chiusa". Chi usa lo strumento puo' **sovrascrivere** solo
cio' che gli serve; il resto resta ai default.

Precedenza (dal piu' forte al piu' debole):

```
flag/param espliciti della superficie  >  env (OBSCURA_*)  >  default in config.rs
```

- **CLI**: i flag sovrascrivono; assenti → default.
- **MCP / HTTP / WS**: i campi presenti nella Request sovrascrivono; assenti →
  default. Un client puo' mandare `{"query":"x"}` e ottenere una ricerca
  ragionevole senza specificare altro.
- Un solo posto (`config.rs`) definisce i default → nessuna divergenza tra
  superfici. La `Request` usa `Option<T>` + `#[serde(default)]`; il merge coi
  default e' una funzione sola (`resolve(request) -> ResolvedConfig`), condivisa.

Default proposti:

| Parametro | Default | Note |
|---|---|---|
| `engine` | `duckduckgo` | endpoint html piu' resiliente |
| `max_results` | `10` | cap rigido di sicurezza |
| `depth` | `serp` | veloce; page/deep on demand |
| `scrape` | `text` (page/deep), `none` (serp) | |
| `concurrency` | `5` | semaforo |
| `wait` | `2` s | settle |
| `timeout` | `20` s | per navigazione |
| `format` | `json` (one-shot), `ndjson` (stream) | |
| monitor `interval` | `60` s | |
| monitor bind | `127.0.0.1` | pubblico richiede `--token` |

---

## 6. Sink di output per-superficie (requisito richiesto)

Il *contenuto* prodotto dal core e' identico ovunque; **il "dove va" dipende dalla
superficie**. Il core scrive su un trait `OutputSink { emit(record); finish(); }`;
ogni superficie fornisce il suo sink:

| Superficie | Modalita' | Dove va l'output |
|---|---|---|
| **CLI** | `--output file` | scrive/append su file (NDJSON incrementale o JSON finale) |
| **CLI** | senza `--output` | stdout (binary-safe; niente banner su `--quiet`) |
| **HTTP** | one-shot `POST` | corpo della risposta JSON quando il core ha finito |
| **HTTP** | streaming `GET`/SSE | ogni record inviato appena pronto (chunked/SSE) |
| **WS** | subscribe | ogni record come frame WS appena pronto (broadcast) |
| **MCP** | `tools/call` | risultato del tool (one-shot) o handle + `_status` in pull |

Regole:
- **Emissione incrementale**: `search` (ndjson/stream) e `monitor` emettono per
  record/evento, non a batch, cosi file-append, SSE e WS vedono i dati in tempo
  reale. `--format json` one-shot bufferizza e chiude alla fine.
- Il core non sa quale sink sta usando → testabile con un `Vec` sink in-memory.
- Backpressure/limiti dei sink di rete gestiti nel sink stesso (sez. 4.2/4.4),
  non nel core.

---

## 7. Riferimenti nel codice (dove agganciarsi)

- CLI: `crates/obscura-cli/src/main.rs` — `enum Command`, `match args.command`,
  pattern `run_fetch` / `run_parallel_scrape` (semaforo, timeout, hard-deadline).
- MCP: `crates/obscura-mcp/src/lib.rs` — `handle_tools_list`, `dispatch`,
  `handle_tool_call`; da riusare: `browser_extract` (estrazione), `browser_search`,
  `browser_links`, `browser_markdown`.
- MCP HTTP: `crates/obscura-mcp/src/http.rs` — `MAX_BODY_BYTES`, origin allowlist,
  CORS: modello per il server di `monitor`.
- Browser: `crates/obscura-browser` — `Page`, `navigate_with_wait`, `settle`,
  `evaluate_with_timeout`, `with_dom`.
- Net/sicurezza: `crates/obscura-net` — `client.rs`, `wreq_client.rs`, gate SSRF,
  robots, blocklist.
- Regole trasversali: `AGENTS.md`.
</content>
