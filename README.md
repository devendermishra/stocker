# NSE stock researcher

Rust workspace that pulls NSE-oriented data from Yahoo Finance (same unofficial endpoints Python **yfinance** uses), runs heuristic analysis (stock, management tone proxy, sector, peers), and exposes:

- **HTTP API** (`stocker-api`, Axum)
- **CLI** (`stocker-cli`)
- **Web UI** (`stocker-web`, Dioxus): **API mode** (WASM + HTTP to the API) or **direct mode** (native desktop, calls `stocker-core` and `stocker-screener` in-process; no API server)

This is not investment advice. Yahoo data can be incomplete or rate-limited.

Further docs: [docs/KNOWLEDGE_BASE.md](docs/KNOWLEDGE_BASE.md) (run commands, env vars, troubleshooting), [docs/KNOWLEDGE_GRAPH.md](docs/KNOWLEDGE_GRAPH.md) (architecture).

## Quick start: desktop app (research + screener)

The desktop build is one native app — research reports and the NSE screener, no API server.

1. Install Rust and Dioxus CLI (see [Prerequisites](#prerequisites)).
2. From the repo root:

```bash
cd frontend
dx serve --platform desktop
```

Without Dioxus CLI:

```bash
cargo run -p stocker-web --no-default-features --features desktop
```

3. In the app:
   - **Generate Report** — enter a symbol (e.g. `RELIANCE`) on the home page
   - **Open Screener** — filter ~110 metrics, save screens, refresh universe data

The screener uses `stocker.db` in the repo root by default. Set `STOCKER_DB_PATH` to override. The refresh scheduler starts when you first open the screener.

Release build: `.\build-standalone.ps1` (Windows) or `./build-standalone.sh` (Unix) → `target/release/stocker-web`.

## Quick start: screener in the browser (WASM + API)

1. **Terminal 1** — start the API (opens `stocker.db`, starts refresh scheduler):

```bash
cargo run -p stocker-api
```

2. **Terminal 2** — start the web UI:

```bash
cd frontend
dx serve --port 8081
```

3. Open `http://127.0.0.1:8081` → click **Open Screener**.

**Windows:** `.\run-dev.ps1` (or `run-dev.bat`) launches both terminals automatically.

Headless alternative (no UI): `cargo run -p stocker-cli -- screener --query query.json` — see [Run the CLI](#run-the-cli).

## Layout

| Path | Role |
|------|------|
| `crates/stocker-core` | Fetch, models, analysis, `build_research_report` |
| `crates/stocker-screener` | NSE screener: SQLite snapshots, metric catalog, refresh job, AND filters |
| `crates/api` | Axum server (research + screener routes) |
| `crates/cli` | Research report JSON + screener queries |
| `frontend` | Dioxus SPA (research + screener UI) |

## Prerequisites

- Rust stable
- For the web app: `wasm32-unknown-unknown` target

```bash
rustup target add wasm32-unknown-unknown
```

Optional (recommended for local web dev):

```bash
cargo install dioxus-cli --locked
```

## Run the API

From the repo root:

```bash
cargo run -p stocker-api
```

Listens on `http://127.0.0.1:8080`.

- Health: `GET http://127.0.0.1:8080/health`
- Report: `GET http://127.0.0.1:8080/api/v1/symbols/RELIANCE/report`  
  (accepts `RELIANCE` or `RELIANCE.NS`; normalized to `*.NS`)

CORS is open for development so the WASM app can call the API from another origin.

## Run the CLI

Research report for one symbol:

```bash
cargo run -p stocker-cli -- report RELIANCE
```

Screener query from a JSON file (uses the same SQLite DB as the API / desktop app):

```bash
cargo run -p stocker-cli -- screener --query query.json
```

Example `query.json` (all filters are combined with **AND**):

```json
{
  "filters": [
    { "field": "return_on_equity", "op": "gte", "value": 0.15 },
    { "field": "pe_ratio", "op": "lte", "value": 25 }
  ],
  "limit": 50
}
```

Refresh one symbol into the screener database:

```bash
cargo run -p stocker-cli -- refresh RELIANCE
```

## NSE stock screener

The screener stores ~110 metrics per NSE symbol in SQLite. The **symbol universe** is loaded from a **local CSV you provide** (Stocker does not call NSE websites or APIs). **Market data** is fetched from Yahoo Finance (same unofficial endpoints as Python **yfinance**). A background job refreshes symbols slowly to avoid rate limits.

### Server mode

See [Quick start: screener in the browser](#quick-start-screener-in-the-browser-wasm--api). Start `stocker-api` (opens `stocker.db` in the repo root by default, starts the refresh scheduler):

```bash
cargo run -p stocker-api
```

Screener HTTP API (same origin as research):

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/screener/fields` | Metric catalog (labels, units, categories) |
| `POST /api/v1/screener/search` | Body: `ScreenQuery`; returns matching rows |
| `GET /api/v1/screener/status` | Universe size, pending refresh count, scheduler state |
| `GET /api/v1/screener/coverage` | Per-metric fill rates |
| `GET/POST/PUT/DELETE /api/v1/screener/screens` | Saved screens CRUD |
| `GET /api/v1/screener/snapshot/{symbol}` | Full SQLite snapshot for one symbol |
| `POST /api/v1/screener/refresh/{symbol}` | Force-refresh one symbol |
| `POST /api/v1/screener/backfill` | Start universe backfill in background |

In the web UI, open **Open Screener** from the home page (WASM build talks to the API).

### Standalone (desktop) mode

See [Quick start: desktop app](#quick-start-desktop-app-research--screener). The desktop build embeds `stocker-screener` in-process: it uses the same `stocker.db` as the API (repo root when you run from the project, or the nearest `stocker.db` found next to the executable / working directory). Set `STOCKER_DB_PATH` to override. The refresh scheduler starts when you first open the screener.

```bash
cargo run -p stocker-web --no-default-features --features desktop
```

No API server required for screener or research in this mode.

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `STOCKER_DB_PATH` | `./stocker.db` (or nearest existing `stocker.db` in a parent directory) | SQLite file location |
| `SCREENER_TIER0_INTERVAL_SECS` | `86400` | NIFTY 500 refresh interval (seconds) |
| `SCREENER_TIER1_INTERVAL_SECS` | `604800` | Rest-of-universe refresh interval |
| `SCREENER_PACING_MS` | `800` | Delay between Yahoo fetches per symbol |
| `SCREENER_BURST_PAUSE_EVERY_N` | `50` | Extra pause after every N symbols |
| `SCREENER_BURST_PAUSE_MS` | `15000` | Length of burst pause (ms) |
| `STOCKER_UNIVERSE_CSV` | *(unset)* | Path to your local universe CSV (required for full-market auto re-sync) |
| `SCREENER_UNIVERSE_SYNC_INTERVAL_SECS` | `86400` | Re-load `STOCKER_UNIVERSE_CSV` when set (local file only; no NSE network) |

### Universe CSV (compliance)

Stocker **does not scrape or call NSE/BSE exchange APIs**. Symbol lists must be **local CSV files** you download yourself (e.g. NSE “Securities available for trading” → `EQUITY_L.csv`). Live quotes and fundamentals come from **Yahoo Finance only**; outbound HTTP in `stocker-core` is enforced by an allowlist (`http_policy.rs`).

```bash
# One-time or after you refresh the file on disk
set STOCKER_UNIVERSE_CSV=D:\data\EQUITY_L.csv
cargo run -p stocker-cli -- universe

# Or per run
cargo run -p stocker-cli -- universe --csv D:\data\EQUITY_L.csv
```

Supported CSV shapes: NSE `EQUITY_L.csv` (`SYMBOL`, optional `SERIES=EQ`), or a single `symbol` column (see bundled `crates/stocker-screener/data/nifty500.csv`). Without a CSV, only the bundled **NIFTY 500** list (~508 names) is used.

### Metric catalog

Filters use snake_case field ids matching the `snapshots` columns (e.g. `pe_ratio`, `return_on_equity`, `sales_growth_3y_cagr_pct`, `altman_z_score`). Composite scores flagged `needs_review` in the API use documented default formulas (Debt Capacity, G Factor, etc.) pending your review.

Shareholding fields (FII/DII/promoter), industry PE/PBV, and all-time prices since 2005 are **not** in v1 (no Yahoo source).

## Web UI: API mode vs direct (standalone) mode

`stocker-web` is one crate with two **mutually exclusive** Cargo features. Both modes include the **screener** (`/screener`) and **research report** (`/report/:symbol`) pages.

| Feature | Target | How data is loaded |
|---------|--------|---------------------|
| **`web`** (default) | `wasm32-unknown-unknown` | Browser UI loads research + screener over **HTTP** from `stocker-api`. |
| **`desktop`** | Native (Windows/macOS/Linux) | Same UI runs **in-process**: `stocker_core` + `stocker_screener`. No API process required. |

See [Quick start: desktop app](#quick-start-desktop-app-research--screener) and [Quick start: screener in the browser](#quick-start-screener-in-the-browser-wasm--api) for step-by-step commands.

Browser WASM cannot link `stocker-core` or `stocker-screener` (networking/SQLite stack is not built for WASM), so the standalone “direct” path is the **native desktop** build, not the same WASM binary.

### API mode (WASM + HTTP)

1. Start the API (see above).
2. From `frontend/`, serve with Dioxus CLI:

```bash
cd frontend
dx serve
```

The SPA defaults to calling `http://127.0.0.1:8080`. To point the WASM build at another API origin, set at **compile time**:

```bash
# PowerShell
$env:STOCKER_API_URL = "http://127.0.0.1:8080"
dx serve
```

```bash
# Unix
STOCKER_API_URL=http://127.0.0.1:8080 dx serve
```

You can also build the WASM artifact only:

```bash
cargo build -p stocker-web --target wasm32-unknown-unknown
```

### Direct mode (standalone desktop)

From `frontend/`, using Dioxus CLI (selects the `desktop` feature and turns off the default `web` feature):

```bash
cd frontend
dx serve --platform desktop
```

Release bundle:

```bash
dx bundle --platform desktop
```

Without the Dioxus CLI, run the desktop binary with Cargo:

```bash
cargo run -p stocker-web --no-default-features --features desktop
```

### Standalone release build (direct mode)

Use this for a **release** standalone app (in-process `stocker-core`; no API server). The workspace root [`Cargo.toml`](Cargo.toml) sets `[profile.release]` (`opt-level = "s"`, LTO).

**Portable binary** (no Dioxus CLI):

```powershell
# Windows (repo root)
.\build-standalone.ps1
```

```bash
# Unix (repo root; mark executable once)
chmod +x build-standalone.sh
./build-standalone.sh
```

Equivalent Cargo invocation:

```bash
cargo build -p stocker-web --release --no-default-features --features desktop
```

Artifact: `target/release/stocker-web` (on Windows, `target/release/stocker-web.exe`).

**Installer-style bundle** (requires [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started/installation)): from the repo root, run the same scripts with the bundle flag, or from `frontend/` run `dx bundle --platform desktop --release --no-default-features`. Bundle output location is printed by `dx` (override with `dx bundle ... --out-dir <dir>`).

```powershell
.\build-standalone.ps1 -Bundle
```

```bash
./build-standalone.sh --bundle
```

On **Windows**, the desktop runtime uses the **WebView2** loader (Evergreen WebView2 is normal on Windows 11; on older systems install the [WebView2 runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) if the app fails to start).

## Run API + UI together (Windows)

From the repo root:

```powershell
.\run-dev.ps1
```

Or:

```bat
run-dev.bat
```

This launches two terminals (API + web UI) and serves the UI at `http://127.0.0.1:8081`.

## Build the whole workspace

```bash
cargo build --workspace
```

## Symbol scope

Inputs are normalized to Yahoo NSE tickers: `SYMBOL.NS`. Use Latin tickers as Yahoo lists them (e.g. `RELIANCE`, `TCS`).
