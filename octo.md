# Obscura Octo — nuove feature

Branch: `octo`

---

## 1. `search` — Cerca con Obscura e scrapperizza

Obscura è un browser. Per cercare:
1. Apre Google (o altro motore) con la query
2. Renderizza la SERP (JS compreso)
3. Estrae i link dei risultati dal DOM
4. (opzionale) Per ogni link, naviga la pagina e la scrapperizza

### Esempio

```
obscura search "ultime news su guerra iran" \
  --engine google                        # motore di ricerca (google, bing)
  --max-results 20                       # quanti risultati estrarre
  --lang it                              # lingua risultati
  --depth serp                           # serp | page | deep
  --format json                          # output format
  --output risultati.json                # salva su file
  --eval "document.title"                # JS da valutare su ogni pagina
  --wait 3                               # secondi di settle dopo load
```

### Engine supportati

| Engine | URL | Note |
|---|---|---|
| `google` | `https://www.google.com/search?q=QUERY&hl=LANG` | Default. Stealth mode aiuta |
| `bing` | `https://www.bing.com/search?q=QUERY&cc=LANG` | Fallback |

### Depth

- `serp`: solo risultati della SERP (link, titolo, snippet)
- `page`: SERP + naviga ogni risultato e dumpa contenuto
- `deep`: SERP + naviga ogni risultato + segue link interni (1 livello)

### Architettura

- Nuovo subcomando `Command::Search` in `main.rs`
- Funzione `run_search()`: naviga → estrai SERP → (opz.) scrape results
- Estrazione SERP via JS injection:
  - Google: `document.querySelectorAll('a[href^="/url?q="]')` oppure `div.g a`
  - Bing: `ol#b_results h2 a`
- Per ogni risultato: riusa logica di `run_fetch` o worker subprocess

---

## 2. `extract` — Estrazione dichiarativa di dati

Naviga una URL e applica uno schema di estrazione dichiarativo (campi definiti da selettori CSS + attributo).

### Esempio CLI

```
obscura extract https://example.com/prodotto \
  --field title:h1@text \
  --field price:.price@text \
  --field link:a.button@href \
  --format json \
  --output prodotto.json
```

### Formato field

```
--field <nome>:<selettore CSS>@<attributo>
```

Attributi: `text` (innerText), `html`, `href`, `src`, `data-*` (qualsiasi attr).

### Config file (YAML)

```yaml
url: https://example.com/prodotto
wait_until: load
wait: 2
fields:
  title:
    selector: h1
    attr: text
  price:
    selector: .price
    attr: text
  link:
    selector: a.button
    attr: href
  immagini:
    selector: img.gallery
    attr: src
    multiple: true
```

```
obscura extract --config schema.yaml --format json
```

### Architettura

- Nuovo subcomando `Command::Extract` in `main.rs`
- Funzione `run_extract()`:
  1. Naviga alla URL
  2. Per ogni field, genera JS snippet: `document.querySelector('<selector>')?.<attr>`
  3. Valuta via `page.evaluate()`
  4. Raccoglie risultati in struct JSON
  5. Output formato richiesto (json, csv, ndjson)

---

## 3. `monitor` — Watch continuo con streaming

Tiene d'occhio una pagina e segnala cambiamenti in tempo reale.

### Esempio

```
obscura monitor https://truthsocial.com/@realDonaldTrump \
  --selector "article:first-child"          # elemento da osservare
  --interval 60                             # secondi tra i poll
  --condizione "textContent.includes('2026')"  # JS condition (truthy = cambiato)
  --on-change "JSON.stringify({text: textContent, time: Date.now()})"  # cosa estrarre
  --save-to trump-watch.jsonl               # append su file
  --stream-ws :9090                         # WebSocket server
  --stream-http :9091                       # HTTP endpoint (ultimo valore)
  --max-runs 0                              # 0 = infinito
```

### Output streaming

- **WS**: broadcast ad ogni cambiamento (JSON message)
- **HTTP GET**: serve l'ultimo valore come JSON
- **File**: appende una riga JSON per cambiamento (NDJSON)

### Architettura

- Nuovo subcomando `Command::Monitor` in `main.rs`
- Funzione `run_monitor()`:
  1. Opzioni: avvia HTTP + WS server su thread separati
  2. Loop:
     a. Naviga alla URL (o aggiorna pagina esistente)
     b. Valuta condizione JS (`condizione`)
     c. Se truthy: valuta `on-change` → confronta con ultimo valore
     d. Se diverso: salva su file, broadcast WS, aggiorna ultimo valore HTTP
     e. Sleep `interval` secondi
- Server HTTP/WS embedded con `tokio::net` + crate leggera (o manuale)

### Watchdog

- Ogni ciclo di navigazione ha timeout (configurabile)
- Se navigazione fallisce: retry dopo `interval`, log errore
- `--max-runs 0` = infinito, altrimenti termina dopo N cicli

---

## Piano implementazione

### Step 1: scheletro comandi in main.rs

Aggiungere `Command::Search`, `Command::Extract`, `Command::Monitor` al enum.

### Step 2: implementare `search`

- `run_search()`: naviga Google con query, estrae link SERP via JS
- `extract_serp_google(dom)`, `extract_serp_bing(dom)`
- Per depth `page`/`deep`: scrape ogni risultato con worker o fetch

### Step 3: implementare `extract`

- `run_extract()`: naviga, per ogni field genera JS, valuta, output
- Supporto CLI flags + file YAML config

### Step 4: implementare `monitor`

- `run_monitor()`: loop poll con HTTP/WS server
- `MonitorState`: tiene traccia ultimo valore, connessioni WS

### Step 5: test

- `cargo build --release` compila pulito
- Test manuali sui tre comandi
- Obstacle course 33/33 (non deve rompere niente)
