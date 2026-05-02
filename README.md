# NSE stock researcher

Rust workspace that pulls NSE-oriented data from Yahoo Finance (same unofficial endpoints Python **yfinance** uses), runs heuristic analysis (stock, management tone proxy, sector, peers), and exposes:

- **HTTP API** (`stocker-api`, Axum)
- **CLI** (`stocker-cli`)
- **Web UI** (`stocker-web`, Dioxus): **API mode** (WASM + HTTP to the API) or **direct mode** (native desktop, calls `stocker-core` in-process; no API server)

This is not investment advice. Yahoo data can be incomplete or rate-limited.

## Layout

| Path | Role |
|------|------|
| `crates/stocker-core` | Fetch, models, analysis, `build_research_report` |
| `crates/api` | Axum server |
| `crates/cli` | JSON to stdout |
| `frontend` | Dioxus SPA |

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

```bash
cargo run -p stocker-cli -- RELIANCE
```

## Web UI: API mode vs direct (standalone) mode

`stocker-web` is one crate with two **mutually exclusive** Cargo features:

| Feature | Target | How data is loaded |
|---------|--------|---------------------|
| **`web`** (default) | `wasm32-unknown-unknown` | Browser UI loads reports over **HTTP** from `stocker-api` (`GET /api/v1/symbols/{symbol}/report`). |
| **`desktop`** | Native (Windows/macOS/Linux) | Same UI runs **in-process**: calls `stocker_core::build_research_report` directly. No API process required. |

Browser WASM cannot link `stocker-core` (networking stack is not built for WASM), so the standalone “direct” path is the **native desktop** build, not the same WASM binary.

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
