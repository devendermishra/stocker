# Stocker Project Knowledge Graph

Operational run commands and troubleshooting: [KNOWLEDGE_BASE.md](KNOWLEDGE_BASE.md).

## Project summary

**stocker** is an NSE-oriented stock research workspace. It fetches data from **Yahoo Finance unofficial HTTP APIs** (the same endpoints Python **yfinance** uses), but implements fetching in Rust via **reqwest** in [`crates/stocker-core/src/fetcher.rs`](../crates/stocker-core/src/fetcher.rs) — there is no Python yfinance dependency.

A **screener** subsystem (`stocker-screener`) stores ~110 metrics per NSE symbol in SQLite, with a tiered background refresh job and AND-filter query engine.

```mermaid
flowchart TB
  subgraph clients [Clients]
    WebUI[stocker-web web WASM]
    DeskUI[stocker-web desktop native]
    CLI[stocker-cli]
    HTTPClient[External HTTP client]
  end

  subgraph api_layer [API layer]
    Axum[stocker-api Axum :8080]
  end

  subgraph domain [Domain rules]
    Domain[stocker-domain]
  end

  subgraph core [Market data]
    Report[build_research_report]
    Fetch[fetcher.rs Yahoo HTTP]
  end

  subgraph screener [Screener subsystem]
    Svc[ScreenerService]
    SQLite[(stocker.db SQLite)]
    Scheduler[RefreshScheduler]
    Query[ScreenQuery engine]
  end

  subgraph external [External]
    Yahoo[Yahoo Finance APIs]
  end

  WebUI -->|GET report + POST screener/search| Axum
  HTTPClient --> Axum
  DeskUI --> Report
  DeskUI --> Svc
  CLI --> Report
  CLI --> Svc
  Axum --> Report
  Axum --> Svc
  Report --> Fetch
  Report --> Domain
  Svc --> SQLite
  Svc --> Scheduler
  Svc --> Query
  Scheduler --> Fetch
  Fetch --> Yahoo
  Query --> Domain
  Fetch --> Domain
```

---

## Workspace entities (crates)

| Node | Path | Role |
|------|------|------|
| **stocker-domain** | [`crates/stocker-domain/`](../crates/stocker-domain/) | Pure rules: research models, scoring, screener catalog and compute, portfolio FIFO, mutual-fund stamp duty. No HTTP, SQLite, or filesystem |
| **stocker-core** | [`crates/stocker-core/`](../crates/stocker-core/) | Yahoo fetch and `build_research_report` orchestration |
| **stocker-screener** | [`crates/stocker-screener/`](../crates/stocker-screener/) | SQLite snapshots, refresh scheduler, SQL for AND filters |
| **stocker-mf** | [`crates/stocker-mf/`](../crates/stocker-mf/) | mfapi.in fetch and `mf.db` |
| **stocker-portfolio** | [`crates/stocker-portfolio/`](../crates/stocker-portfolio/) | `portfolio.db`, import, and ledger persistence |
| **stocker-sync** | [`crates/stocker-sync/`](../crates/stocker-sync/) | Drive vault backup |
| **stocker-api** | [`crates/api/`](../crates/api/) | Axum HTTP server on `127.0.0.1:8080` (research + screener routes) |
| **stocker-cli** | [`crates/cli/`](../crates/cli/) | Headless: reports, screener queries, universe sync, backfill |
| **stocker-web** | [`frontend/`](../frontend/) | Dioxus UI — research + screener (not Tauri, not npm). Both modes share `stocker-domain` types |

Workspace root: [`Cargo.toml`](../Cargo.toml)

---

## How to run (summary)

| Goal | Command |
|------|---------|
| **Desktop app** (research + screener, no API) | `cd frontend && dx serve --platform desktop` |
| **Browser UI + screener** | Terminal 1: `cargo run -p stocker-api` · Terminal 2: `cd frontend && dx serve --port 8081` |
| **Windows both-at-once** | `.\run-dev.ps1` |
| **CLI screener query** | `cargo run -p stocker-cli -- screener --query query.json` |
| **Release desktop binary** | `.\build-standalone.ps1` or `./build-standalone.sh` |

See [KNOWLEDGE_BASE.md](KNOWLEDGE_BASE.md) for full steps, env vars, and troubleshooting.

---

## Backend architecture

### Domain modules ([`stocker-domain/src/`](../crates/stocker-domain/src/))

Rules only (`serde`, `serde_json`, `thiserror`, `chrono` dates). Callers pass `as_of`; the clock stays in the orchestrator.

| Module | Responsibility |
|--------|----------------|
| `models.rs` / `report.rs` | Serde types, including `ResearchReport` |
| `analysis.rs`, `valuation_analysis.rs`, `fundamental_analysis.rs`, `technical_analysis.rs`, `technical_entry_signal.rs`, `stock_scoring.rs`, `research_summary.rs` | Research heuristics |
| `sector_research.rs` | Porter Five Forces, lifecycle, sector type, demand–supply, competition, profitability, growth |
| `math.rs`, `statements.rs`, `ratios.rs`, `canonical.rs`, `series_integrity.rs` | Shared calculations |
| `symbol.rs` | Ticker normalization and NSE CSV *bytes* parsing |
| `screener/` | Metric catalog, `compute_all`, and the `ScreenQuery` model |
| `portfolio/` | Transaction types and pure FIFO replay |
| `mf/` | Stamp duty, trading-day calendar, `MF:{code}` symbols |

### Core modules ([`stocker-core/src/`](../crates/stocker-core/src/))

| Module | Responsibility |
|--------|----------------|
| `fetcher.rs` | Yahoo HTTP, crumb auth, JSON parsing |
| `yahoo_metrics.rs` | Map Yahoo JSON into domain financials |
| `symbol.rs` | Load `data/EQUITY_L.csv` from disk |
| `report.rs` | Orchestration: parallel fetch → domain analysis → `ResearchReport` |
| `bank_metrics.rs` | Bank metric CSV load |
| `sector.rs` | Sector outlook HTTP |
| `paths.rs` | Database and config paths |

### Screener modules ([`stocker-screener/src/`](../crates/stocker-screener/src/))

| Module | Responsibility |
|--------|----------------|
| `db.rs` | SQLite pool, migrations, schema validation |
| `snapshot.rs` | Upsert symbol + metric rows from Yahoo fetch |
| `query.rs` | Render a domain `ScreenQuery` as SQL |
| `refresh.rs` | Tiered scheduler, pacing, backfill |
| `service.rs` | Public façade (`ScreenerService`) for API + desktop |
| `screens.rs` | Saved screen CRUD |
| `universe.rs` | Load symbols from local CSV (no NSE network) |

### API surface ([`crates/api/src/lib.rs`](../crates/api/src/lib.rs))

**Research:**

- `GET /health` → `{"status":"ok"}`
- `GET /api/v1/symbols/{symbol}/report` → `ResearchReport` JSON

**Screener** ([`crates/api/src/screener.rs`](../crates/api/src/screener.rs)):

- `GET /api/v1/screener/fields` — metric catalog
- `POST /api/v1/screener/search` — `ScreenQuery` → matching rows
- `GET /api/v1/screener/status` — scheduler + universe stats
- `GET /api/v1/screener/coverage` — per-metric fill rates
- `GET/POST/PUT/DELETE /api/v1/screener/screens` — saved screens
- `GET /api/v1/screener/snapshot/{symbol}` — full snapshot for one symbol
- `POST /api/v1/screener/refresh/{symbol}` — force-refresh one symbol
- `POST /api/v1/screener/backfill` — universe backfill (409 if already running)
- `POST /api/v1/screener/recompute` — recompute composites
- `POST /api/v1/screener/scheduler/stop` — stop scheduler

**Sector research** ([`crates/api/src/sectors.rs`](../crates/api/src/sectors.rs)):

- `GET /api/v1/sectors` — distinct Yahoo sectors + summary badges (lifecycle, type, attractiveness, growth)
- `GET /api/v1/sectors/{sector}` — full heuristic `SectorResearchProfile` + top members by mcap

Profiles are **computed on read** from screener `symbols` + `snapshots` (no separate sector cache). Heuristics live in [`crates/stocker-domain/src/sector_research.rs`](../crates/stocker-domain/src/sector_research.rs): Porter Five Forces, lifecycle, sector type, demand–supply gap, competition, profitability, growth prospects, supplier/customer pricing power.

### Entry points → core / screener

| Entry | Research | Screener |
|-------|----------|----------|
| CLI | `stocker_core::build_research_report` | `ScreenerService` (open DB, query, backfill) |
| API | Same via [`handlers.rs`](../crates/api/src/handlers.rs) | Same via [`screener.rs`](../crates/api/src/screener.rs) |
| Desktop UI | [`frontend/src/api.rs`](../frontend/src/api.rs) (`feature = "desktop"`) | [`frontend/src/screener_api.rs`](../frontend/src/screener_api.rs) (`feature = "desktop"`) |
| Web UI | HTTP to API (`feature = "web"`) | HTTP to API (`feature = "web"`) |

---

## Frontend: web vs desktop (standalone)

Single crate **stocker-web** with **mutually exclusive** Cargo features ([`frontend/Cargo.toml`](../frontend/Cargo.toml), [`frontend/src/main.rs`](../frontend/src/main.rs)):

```mermaid
flowchart LR
  subgraph webMode [Web mode feature web]
    Browser[Browser WASM]
    Gloo[gloo-net HTTP]
    API[stocker-api :8080]
  end

  subgraph deskMode [Standalone feature desktop]
    Native[Native Dioxus Desktop]
    CoreDirect[stocker_core in-process]
    ScreenerDirect[stocker_screener in-process]
  end

  Browser --> Gloo --> API
  API --> CoreDirect
  API --> ScreenerDirect
  Native --> CoreDirect
  Native --> ScreenerDirect
```

| Mode | Feature | Target | Backend connection | API required? |
|------|---------|--------|-------------------|---------------|
| **Web / API mode** | `web` (default) | `wasm32-unknown-unknown` | HTTP to `stocker-api` (research + screener). Types come from `stocker-domain` | Yes |
| **Desktop / standalone** | `desktop` | Native OS binary | `stocker_core` + `stocker_screener` + `stocker_portfolio` in-process, sharing `stocker-domain` types | No |

Central switches:

- **Research:** [`frontend/src/api.rs`](../frontend/src/api.rs) — web: `gloo_net::Request::get`; desktop: `stocker_core::build_research_report`
- **Screener:** [`frontend/src/screener_api.rs`](../frontend/src/screener_api.rs) — web: HTTP to `/api/v1/screener/*`; desktop: `ScreenerService` methods

WASM links `stocker-domain` for the shared report and screener types. It does not link `stocker-core` or `stocker-screener` (no native HTTP/SQLite stack on wasm32); standalone direct mode is **native desktop only**.

### UI routes and tabs

- Routes: [`frontend/src/routes.rs`](../frontend/src/routes.rs) — `/` (home), `/report/:symbol`, `/screener`
- Report page: **9 tabs** — Overview, Research, Financials, **Detailed Data** (SQLite metrics), Sector (incl. heuristic sector research), Peers, News, Management, Framework ([`frontend/src/report/mod.rs`](../frontend/src/report/mod.rs))
- Sector catalog: `/sectors`, `/sectors/:sector` ([`frontend/src/sectors/`](../frontend/src/sectors/))
- Screener page: filter builder, results, saved screens, coverage tab, refresh/backfill button ([`frontend/src/screener/mod.rs`](../frontend/src/screener/mod.rs))

---

## Screener data flow

```mermaid
sequenceDiagram
  participant User
  participant UI as stocker-web or CLI
  participant Svc as ScreenerService
  participant Sched as RefreshScheduler
  participant Fetch as stocker-core fetcher
  participant DB as stocker.db
  participant Yahoo as Yahoo Finance

  User->>UI: Open /screener or run query
  UI->>Svc: search / status / backfill
  Svc->>DB: SQL query or read status

  Note over Sched: Started by API boot or first screener page open (desktop)
  Sched->>DB: Pick due symbol (tier 0 / tier 1)
  Sched->>Fetch: Fetch Yahoo quoteSummary modules
  Fetch->>Yahoo: HTTP
  Yahoo-->>Fetch: JSON
  Fetch-->>Sched: Parsed fields
  Sched->>Svc: compute + upsert snapshot
  Svc->>DB: UPDATE snapshots

  User->>UI: Apply filters
  UI->>Svc: ScreenQuery
  Svc->>DB: SELECT with AND WHERE
  DB-->>UI: ScreenRow list
```

### Screener user journey (UI)

```mermaid
flowchart TD
  A[Open app] --> B[Home]
  B --> C[Generate Report → /report/:symbol]
  B --> D[Open Screener → /screener]
  D --> E[Build filters AND]
  E --> F[Run search]
  F --> G[View results / save screen]
  D --> H[Refresh stock data backfill]
  H --> I{Already running?}
  I -->|yes| J[Show message]
  I -->|no| K[Background backfill]
  C --> L[Detailed Data tab]
  L --> M[All SQLite metrics for symbol]
```

---

## Yahoo / yfinance data source

**Clarification:** The project does not call the yfinance Python library. It calls Yahoo's unofficial REST APIs directly (documented in README as "same endpoints Python yfinance uses").

### Endpoints used

| Endpoint | Purpose |
|----------|---------|
| `query2.finance.yahoo.com/v10/finance/quoteSummary/{symbol}?modules=...` | Price, financials, profile, statements, holders |
| `query1.finance.yahoo.com/v8/finance/chart/{symbol}?interval=1d&range=5y` | OHLCV history |
| `query2.finance.yahoo.com/v1/finance/search` | Company news, sector news, NSE peer discovery |
| Crumb flow: `fc.yahoo.com` → `finance.yahoo.com` → `getcrumb` | Auth cookie + crumb for quoteSummary |

### quoteSummary modules requested (by fetch function)

| Fetch function | Yahoo `modules` |
|----------------|-----------------|
| `fetch_price` | `price` |
| `fetch_financials` | `financialData,defaultKeyStatistics,summaryDetail,price` |
| `fetch_shareholders` | `majorHoldersBreakdown,netSharePurchaseActivity` |
| `fetch_market_signals` | `recommendationTrend,insiderTransactions,institutionOwnership` |
| `fetch_annual_reports` | `incomeStatementHistory` |
| `fetch_officer_pay` / profile | `assetProfile` |
| `fetch_asset_profile` | `assetProfile,price` |
| `fetch_statement_bundle` | `incomeStatementHistory,incomeStatementHistoryQuarterly,balanceSheetHistory,balanceSheetHistoryQuarterly,cashflowStatementHistory,cashflowStatementHistoryQuarterly` |
| `fetch_peer_quotes` | `price,financialData,summaryDetail,assetProfile,defaultKeyStatistics` |

---

## Scraped values (Yahoo JSON fields → app models)

Values are read from nested JSON, typically `.raw` (numbers) or `.fmt` (dates/strings).

### Price module → quote / `Financials` (partial)

`regularMarketPrice`, `regularMarketChangePercent`, `regularMarketPreviousClose`, `fiftyTwoWeekHigh`, `fiftyTwoWeekLow`, `marketCap`, `regularMarketVolume`, `averageDailyVolume10Day`, `longName`, `shortName`, `symbol`, `exchange`, `fullExchangeName`, `currency`

### financialData module → `Financials`

`totalRevenue`, `netIncomeToCommon`, `totalDebt`, `ebitda`, `profitMargins`, `grossMargins`, `operatingMargins`, `ebitdaMargins`, `returnOnEquity`, `returnOnAssets`, `returnOnCapital`, `returnOnInvestedCapital`, `debtToEquity`, `freeCashflow`, `operatingCashflow`, `enterpriseValue`, `totalCash`, `revenueGrowth`, `earningsGrowth`

### summaryDetail module → `Financials`

`trailingPE`, `forwardPE`, `previousClose`, `dividendYield`, `payoutRatio`, `beta`, `priceToSalesTrailing12Months`, `priceToBook`, `exDividendDate`

### defaultKeyStatistics module → `Financials`

`bookValue`, `priceToBook`, `trailingEps`, `forwardEps`, `sharesOutstanding`, `beta`, `priceToSalesTrailing12Months`

### incomeStatementHistory (annual rows) → `AnnualReport` / statements

`endDate`, `totalRevenue`, `costOfRevenue`, `grossProfit`, `ebitda`, `operatingIncome`, `netIncome`, `dilutedEPS`, `basicEPS`

### balanceSheetHistory → `BalanceSheetRow`

`endDate`, `cash`, `cashAndCashEquivalents`, `otherShortTermInvestments`, `totalDebt`, `longTermDebt`, `shortLongTermDebtTotal`, `currentDebt`, `totalStockholderEquity`, `commonStockTotalEquity`, `totalAssets`, `totalLiab`, `totalLiabilitiesNetMinorityInterest`, `currentAssets`, `currentLiabilities`, `interestExpense`, `inventory`, `netReceivables`

### cashflowStatementHistory → `CashflowRow`

`endDate`, `totalCashFromOperatingActivities`, `operatingCashflow`, `capitalExpenditures`, `freeCashflow`

### majorHoldersBreakdown / netSharePurchaseActivity → `Shareholders`

`insidersPercentHeld`, `institutionsPercentHeld`, `institutionsFloatPercentHeld`, `heldPercentInsiders`, `netInfoShares`

### assetProfile → `AssetProfile` / management proxy

`sector`, `industry`, `longBusinessSummary`, `website`, `country`, `longName`, `companyOfficers[].totalPay`

### Chart API → `ChartBar` / technical analysis

`timestamp`, `indicators.quote[0].open`, `high`, `low`, `close`, `volume` (5y daily range in report build)

### Search API → `NewsItem` / peers

- **News:** `news[].title`, `link`, `providerPublishTime`
- **Peers:** `quotes[].symbol`, `quoteType` (filter `EQUITY`, suffix `.NS`)

---

## High-level report generation flow

```mermaid
sequenceDiagram
  participant User
  participant UI as stocker-web or CLI or API
  participant Report as build_research_report
  participant Fetch as fetcher
  participant Yahoo as Yahoo Finance
  participant Analyze as analysis modules

  User->>UI: symbol e.g. RELIANCE
  UI->>Report: build_research_report
  Report->>Report: normalize_nse_symbol to RELIANCE.NS

  par Parallel fetch
    Report->>Fetch: fetch_price, financials, shareholders, annual_reports, officer_pay, asset_profile, statements, chart 5y
    Fetch->>Yahoo: quoteSummary + chart
    Yahoo-->>Fetch: JSON
  end

  Report->>Fetch: fetch_company_news, sector_news
  Report->>Fetch: discover_nse_peer_symbols
  Report->>Fetch: fetch_peer_quotes

  Report->>Analyze: stock, management, sector, peer, fundamental, valuation, technical, scoring
  Analyze-->>Report: analysis structs
  Report-->>UI: ResearchReport JSON
  UI-->>User: 9-tab report UI or stdout
```

### Pipeline stages inside `build_research_report` ([`report.rs`](../crates/stocker-core/src/report.rs))

1. **Normalize** symbol to `*.NS` via [`stocker-domain` `symbol.rs`](../crates/stocker-domain/src/symbol.rs) (NSE universe file load stays in [`stocker-core` `symbol.rs`](../crates/stocker-core/src/symbol.rs))
2. **Parallel fetch** (8 calls via `tokio::join!`): price, financials, shareholders, annual reports, officer pay, asset profile, statement bundle, 5y chart
3. **Enrich** net income from annual reports if Yahoo TTM is ≤ 0
4. **News**: company news (search by symbol/name/sector/industry), sector news
5. **Peers**: discover NSE peer symbols → batch `fetch_peer_quotes`
6. **Analyze**: stock, management, sector, peer, insights, structured sections, fundamentals, valuation, technical, entry signal, research rating, summary
7. **Return** `ResearchReport` with ~20 top-level fields (symbol, price, financials, analyses, news, ratings, etc.)

### User journey (research UI)

```mermaid
flowchart TD
  A[Open app] --> B[Home / enter symbol]
  B --> C[Generate Report]
  C --> D["/report/:symbol"]
  D --> E{load_research_report}
  E -->|web| F[HTTP to stocker-api]
  E -->|desktop| G[stocker_core in-process]
  F --> H[build_research_report]
  G --> H
  H --> I[Render 9 analysis tabs incl. Detailed Data]
  I --> J[Back to Home]
```

---

## Knowledge graph node index (for reference)

**Components:** `stocker-core`, `stocker-screener`, `stocker-api`, `stocker-cli`, `stocker-web`  
**Modes:** `web+HTTP`, `desktop-standalone`, `cli-headless`  
**External:** `YahooFinance` (quoteSummary, chart, search, crumb)  
**Key artifacts:** `ResearchReport`, `Financials`, `ChartHistory`, `PeerQuote`, `NewsItem`, `ScreenRow`, `ScreenQuery`  
**Key processes:** `build_research_report`, `RefreshScheduler`, `ScreenQuery`  
**UI surfaces:** Home, Report (9 tabs), Screener (filters + coverage), Sector Research (`/sectors`)  
**Docs:** [KNOWLEDGE_BASE.md](KNOWLEDGE_BASE.md), [README.md](../README.md)
