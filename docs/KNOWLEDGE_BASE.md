# Stocker Knowledge Base

Operational reference for running, configuring, and understanding the Stocker workspace. For architecture diagrams and module relationships, see [KNOWLEDGE_GRAPH.md](KNOWLEDGE_GRAPH.md).

---

## What Stocker does

Stocker is an NSE-oriented stock research workspace written in Rust. It:

- Fetches market and fundamental data from **Yahoo Finance** (same unofficial endpoints Python **yfinance** uses)
- Builds heuristic research reports (valuation, technicals, peers, management proxy, etc.)
- Maintains an **NSE stock screener** in SQLite (~110 metrics per symbol, AND filters, saved screens)

**Not investment advice.** Yahoo data can be incomplete or rate-limited.

---

## Workspace layout

| Path | Crate / role |
|------|----------------|
| `crates/stocker-core` | Fetch, models, analysis, shared `math` / `statements` helpers, `build_research_report` |
| `crates/stocker-screener` | SQLite snapshots, metric catalog, refresh scheduler, query engine |
| `crates/api` | Axum HTTP server (`stocker-api`) — research + screener routes |
| `crates/cli` | Headless CLI — reports, screener queries, universe sync, backfill |
| `frontend` | Dioxus UI (`stocker-web`) — research + screener in one app |

---

## Prerequisites

1. **Rust stable** — [rustup.rs](https://rustup.rs)
2. **WASM target** (web mode only):

   ```bash
   rustup target add wasm32-unknown-unknown
   ```

3. **Dioxus CLI** (recommended for web and desktop dev):

   ```bash
   cargo install dioxus-cli --locked
   ```

4. **WebView2** (Windows desktop only) — preinstalled on Windows 11; on older systems install the [WebView2 runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) if the desktop app fails to launch.

---

## Run the desktop app (research + screener)

The **desktop build** is a single native app. It calls `stocker-core` and `stocker-screener` **in-process** — no API server required.

### Development (hot reload)

From the repo root:

```bash
cd frontend
dx serve --platform desktop
```

Without Dioxus CLI:

```bash
cargo run -p stocker-web --no-default-features --features desktop
```

### Release binary

```powershell
# Windows
.\build-standalone.ps1
```

```bash
# Unix
chmod +x build-standalone.sh
./build-standalone.sh
```

Artifact: `target/release/stocker-web` (Windows: `stocker-web.exe`).

### What you can do in the desktop app

| Route | Action |
|-------|--------|
| `/` (Home) | Enter a symbol → **Generate Report**, or click **Open Screener** |
| `/report/:symbol` | 9-tab research report (Overview, Research, Financials, **Detailed Data**, Sector, Peers, News, Management, Framework) |
| `/screener` | Filter builder, results table, saved screens, coverage tab, **Refresh stock data** (universe backfill) |

### Desktop screener database

- Default SQLite file: `stocker.db` in the repo root (or nearest `stocker.db` found upward from the working directory / executable).
- Override with `STOCKER_DB_PATH`.
- The refresh scheduler starts when you first open the screener page.
- Optional full-market universe: set `STOCKER_UNIVERSE_CSV` to a local NSE symbol CSV (see [Universe CSV](#universe-csv) below).

---

## Run the screener app

The screener is available in **three** ways: inside the desktop app, inside the browser (WASM + API), or headless via CLI.

### Option A — Desktop app (simplest)

Follow [Run the desktop app](#run-the-desktop-app-research--screener) above, then click **Open Screener** on the home page. No separate process.

### Option B — Browser (WASM + API)

Requires two terminals: API server + web UI.

**Terminal 1 — API** (opens `stocker.db`, starts refresh scheduler):

```bash
cargo run -p stocker-api
```

Listens on `http://127.0.0.1:8080`.

**Terminal 2 — Web UI**:

```bash
cd frontend
dx serve --port 8081
```

Open `http://127.0.0.1:8081` → **Open Screener**.

**Windows shortcut** — one command launches both:

```powershell
.\run-dev.ps1
```

Or `run-dev.bat`.

The WASM build talks to the API for both research reports and screener endpoints. Set `STOCKER_API_URL` at compile time if the API is not on `http://127.0.0.1:8080`:

```powershell
$env:STOCKER_API_URL = "http://127.0.0.1:8080"
dx serve --port 8081
```

### Option C — CLI (headless)

Query the same SQLite database without a UI:

```bash
# Run a filter query from JSON
cargo run -p stocker-cli -- screener --query query.json

# Refresh one symbol
cargo run -p stocker-cli -- refresh RELIANCE

# Load universe from CSV
cargo run -p stocker-cli -- universe --csv D:\data\EQUITY_L.csv

# DB + scheduler status
cargo run -p stocker-cli -- status

# Foreground backfill (all symbols)
cargo run -p stocker-cli -- backfill
```

Example `query.json`:

```json
{
  "filters": [
    { "field": "return_on_equity", "op": "gte", "value": 0.15 },
    { "field": "pe_ratio", "op": "lte", "value": 25 }
  ],
  "limit": 50
}
```

---

## Screener HTTP API

When `stocker-api` is running with a valid `stocker.db`:

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `GET` | `/api/v1/screener/fields` | Metric catalog |
| `POST` | `/api/v1/screener/search` | Run `ScreenQuery` (AND filters) |
| `GET` | `/api/v1/screener/status` | Universe size, pending refresh, scheduler state |
| `GET` | `/api/v1/screener/coverage` | Per-metric fill rates |
| `GET/POST/PUT/DELETE` | `/api/v1/screener/screens` | Saved screens CRUD |
| `GET` | `/api/v1/screener/snapshot/{symbol}` | Full snapshot for one symbol |
| `POST` | `/api/v1/screener/refresh/{symbol}` | Force-refresh one symbol |
| `POST` | `/api/v1/screener/backfill` | Start universe backfill in background (409 if already running) |
| `POST` | `/api/v1/screener/recompute` | Recompute composite metrics |
| `POST` | `/api/v1/screener/scheduler/stop` | Stop refresh scheduler |

Research API (same server): `GET /api/v1/symbols/{symbol}/report`, `GET /health`.

---

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `STOCKER_DB_PATH` | `./stocker.db` (or nearest existing `stocker.db` in a parent directory) | SQLite file location |
| `STOCKER_UNIVERSE_CSV` | *(unset)* | Path to local universe CSV (required for full-market auto re-sync) |
| `STOCKER_API_URL` | `http://127.0.0.1:8080` | API origin for WASM build (compile-time) |
| `SCREENER_TIER0_INTERVAL_SECS` | `86400` | NIFTY 500 refresh interval (seconds) |
| `SCREENER_TIER1_INTERVAL_SECS` | `604800` | Rest-of-universe refresh interval |
| `SCREENER_PACING_MS` | `800` | Delay between Yahoo fetches per symbol |
| `SCREENER_BURST_PAUSE_EVERY_N` | `50` | Extra pause after every N symbols |
| `SCREENER_BURST_PAUSE_MS` | `15000` | Length of burst pause (ms) |
| `SCREENER_UNIVERSE_SYNC_INTERVAL_SECS` | `86400` | Re-load `STOCKER_UNIVERSE_CSV` when set |
| `STOCKER_PORTFOLIO_DB_PATH` | `./portfolio.db` (or nearest existing file) | Portfolio SQLite location |
| `STOCKER_GOOGLE_CLIENT_ID` | *(unset)* | Google OAuth desktop client ID for Drive sync (dev bypass; skips encrypted vault) |
| `STOCKER_GOOGLE_CLIENT_SECRET` | *(unset)* | Google OAuth desktop client secret for Drive sync (dev bypass) |
| `STOCKER_CONFIG_DIR` | `~/.config/stocker` (platform-specific) | Encrypted sync vault, legacy tokens, and sync state |

---

## Google Drive database sync

Sync **`portfolio.db`** and **`stocker.db`** between devices via a single backup zip in your Google Drive **app data folder** (hidden from the normal Drive UI). `mf.db` is not synced (MF NAV cache; refresh locally).

### One-time Google Cloud setup

1. Create a project in [Google Cloud Console](https://console.cloud.google.com/).
2. Enable the **Google Drive API**.
3. Create **OAuth client ID** → Application type **Desktop app**.
4. Add redirect URI `http://127.0.0.1` (the app binds a random port at auth time; Google accepts loopback redirects for desktop clients).
5. **Desktop app (recommended):** open **Sync** in the app → the setup dialog collects client ID, client secret, and a **master password**. All sync secrets (OAuth credentials, Google tokens, sync state) are stored in an encrypted vault at `~/.config/stocker/sync_vault.enc` (Argon2id + ChaCha20-Poly1305). Unlock the vault each session before signing in or syncing.
6. On the OAuth consent screen configuration, add scope **`https://www.googleapis.com/auth/drive.appdata`** (Data from Google Drive apps) and enable the **Google Drive API** under APIs & Services.
7. **Dev bypass (optional):** set `STOCKER_GOOGLE_CLIENT_ID` and `STOCKER_GOOGLE_CLIENT_SECRET` env vars — credentials are not encrypted and legacy plaintext token/state files are used until you migrate via the setup dialog.

Legacy plaintext files (`google_oauth.json`, `google_tokens.json`, `sync_state.json`) are imported into the vault on first setup or unlock, then deleted.

### CLI workflow

```bash
# Sign in (opens browser)
cargo run -p stocker-cli -- sync auth

# Check local vs remote timestamps
cargo run -p stocker-cli -- sync status

# Smart sync: pull if Drive is newer, else push if local changed
cargo run -p stocker-cli -- sync run

# Force upload or download
cargo run -p stocker-cli -- sync push --force
cargo run -p stocker-cli -- sync pull --force
```

Close the desktop app before `sync pull` from the CLI (database files must not be open). The desktop app runs an automatic pull on startup when Drive has a newer backup.

### Multi-device workflow

1. **Device A** — use the app, then `sync run` (or **Sync now** on the home page → **Sync**).
2. **Device B** — launch the desktop app (startup pull) or run `sync run`.
3. If both devices edited since the last sync, status shows a **conflict** — choose `sync push --force` or `sync pull --force`.

Local encrypted vault: `~/.config/stocker/sync_vault.enc`. Legacy plaintext paths are migrated automatically on vault setup/unlock.

---

## Universe CSV

Stocker **does not scrape or call NSE/BSE exchange APIs**. Symbol lists must be **local CSV files** you download yourself (e.g. NSE “Securities available for trading” → `EQUITY_L.csv`).

```powershell
# One-time or after you refresh the file on disk
$env:STOCKER_UNIVERSE_CSV = "D:\data\EQUITY_L.csv"
cargo run -p stocker-cli -- universe
```

Supported CSV shapes: NSE `EQUITY_L.csv` (`SYMBOL`, optional `SERIES=EQ`), or a single `symbol` column. Without a CSV, only the bundled **NIFTY 500** list (~508 names) in `crates/stocker-screener/data/nifty500.csv` is used.

---

## Web vs desktop feature matrix

| Capability | Web (WASM + API) | Desktop (native) |
|------------|------------------|------------------|
| Research report | HTTP → `stocker-api` | In-process `stocker_core` |
| Screener search / saved screens | HTTP → `stocker-api` | In-process `stocker_screener` |
| Screener refresh scheduler | Started by API on boot | Started when screener page opens |
| Google Drive DB sync | No | Yes (CLI + Sync page; startup pull) |
| Requires API server | Yes | No |
| Cargo feature | `web` (default) | `desktop` |

Central switch: `frontend/src/api.rs` (research) and `frontend/src/screener_api.rs` (screener).

---

## First-time screener setup checklist

1. Clone repo and install prerequisites above.
2. Choose a mode:
   - **Desktop only** → `dx serve --platform desktop` → Open Screener.
   - **Browser** → `cargo run -p stocker-api` + `dx serve --port 8081` (or `run-dev.ps1`).
3. (Optional) Load a full universe CSV via CLI or set `STOCKER_UNIVERSE_CSV`.
4. Wait for initial data: the scheduler refreshes symbols slowly (pacing env vars). Use **Refresh stock data** on the screener page for a full backfill, or `cargo run -p stocker-cli -- backfill` in the foreground.
5. Check progress: screener status footer, `GET /api/v1/screener/status`, or `cargo run -p stocker-cli -- status`.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Desktop app won't start (Windows) | Missing WebView2 | Install [WebView2 runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) |
| Screener shows empty / no rows | DB not populated yet | Run backfill or wait for scheduler; check `status` |
| WASM screener errors / CORS | API not running | Start `stocker-api` first |
| "Already running" on refresh | Backfill in progress | Wait for current job; status shows `backfill_running` |
| Only ~508 symbols | No universe CSV | Set `STOCKER_UNIVERSE_CSV` and run `universe` |
| Drive sync upload 403 | Missing `drive.appdata` scope on token, or Drive API disabled | Enable Drive API; add `drive.appdata` to OAuth consent screen scopes; use **Sign out** then **Sign in with Google** on Sync page |
| Drive sync auth fails | Missing OAuth credentials or vault locked | Use Sync page setup dialog, or set env vars; unlock vault with master password |
| Drive sync pull fails (CLI) | DB files open | Close desktop app, then `sync pull` |
| Drive sync conflict | Both devices edited | `sync push --force` or `sync pull --force` |
| `dx` not found | Dioxus CLI missing | `cargo install dioxus-cli --locked` |

---

## Shared backend utilities

Financial math (`pct_change`, `cagr`, `median`) and statement sorting helpers live in `stocker-core` (`math.rs`, `statements.rs`) so analysis modules and the screener metric engine share one implementation. The screener's `compute_all` function delegates to section builders (price, valuation, balance sheet, technical, composites, etc.) rather than one monolithic block.

---

## Related docs

- [README.md](../README.md) — project overview and all run commands
- [KNOWLEDGE_GRAPH.md](KNOWLEDGE_GRAPH.md) — architecture, data flow, module index
