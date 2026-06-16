//! Parse and import portfolio transactions from CSV / XLS / XLSX files.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use stocker_core::symbol::{default_india_symbol_context, resolve_india_symbol, IndiaSymbolContext};
use stocker_mf::{load_scheme_index_from_file, default_scheme_list_cache_path, is_mutual_fund_symbol, MfService, SchemeIndex};

use crate::engine::rebuild;
use crate::error::{Error, Result};
use crate::models::{NewTransaction, TransactionType};
use crate::portfolios;

/// Mappable transaction field labels for the import wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportField {
    Skip,
    TxnType,
    TradeDate,
    Symbol,
    StockName,
    Isin,
    Quantity,
    Price,
    GrossAmount,
    Brokerage,
    Taxes,
    NetAmount,
    SplitRatioNum,
    SplitRatioDen,
    BonusRatioNum,
    BonusRatioDen,
    DividendPerShare,
    Tds,
    EligibleQuantity,
    Notes,
}

impl ImportField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skip => "— Skip —",
            Self::TxnType => "Transaction Type",
            Self::TradeDate => "Trade Date",
            Self::Symbol => "Symbol",
            Self::StockName => "Stock / MF / ETF Name",
            Self::Isin => "ISIN",
            Self::Quantity => "Quantity",
            Self::Price => "Price",
            Self::GrossAmount => "Amount",
            Self::Brokerage => "Brokerage",
            Self::Taxes => "Taxes",
            Self::NetAmount => "Net Amount",
            Self::SplitRatioNum => "Split Ratio (num)",
            Self::SplitRatioDen => "Split Ratio (den)",
            Self::BonusRatioNum => "Bonus Ratio (num)",
            Self::BonusRatioDen => "Bonus Ratio (den)",
            Self::DividendPerShare => "Dividend / Share",
            Self::Tds => "TDS",
            Self::EligibleQuantity => "Eligible Quantity",
            Self::Notes => "Notes",
        }
    }

    pub fn all_mappable() -> &'static [ImportField] {
        &[
            Self::Skip,
            Self::TxnType,
            Self::TradeDate,
            Self::Symbol,
            Self::StockName,
            Self::Isin,
            Self::Quantity,
            Self::Price,
            Self::GrossAmount,
            Self::Brokerage,
            Self::Taxes,
            Self::NetAmount,
            Self::SplitRatioNum,
            Self::SplitRatioDen,
            Self::BonusRatioNum,
            Self::BonusRatioDen,
            Self::DividendPerShare,
            Self::Tds,
            Self::EligibleQuantity,
            Self::Notes,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGrid {
    pub rows: Vec<Vec<String>>,
    pub sheet_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePreview {
    pub grid: RawGrid,
    pub suggested_header_row: usize,
    pub suggested_mapping: Vec<ImportField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowPreview {
    pub row_index: usize,
    pub transaction: Option<NewTransaction>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportParseBody {
    pub filename: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportApplyRequest {
    pub header_row: usize,
    pub column_mapping: Vec<ImportField>,
    pub grid: RawGrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<ImportRowError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row_index: usize,
    pub message: String,
}

struct SymbolResolver {
    isin_to_symbol: HashMap<String, String>,
    name_to_symbol: HashMap<String, String>,
    india_ctx: IndiaSymbolContext,
    mf_index: SchemeIndex,
}

impl SymbolResolver {
    fn new() -> Self {
        let mut isin_to_symbol = HashMap::new();
        let mut name_to_symbol = HashMap::new();
        load_isin_index("data/EQUITY_L.csv", true, &mut isin_to_symbol, &mut name_to_symbol);
        load_isin_index("data/EQUITY_L_BSE.csv", false, &mut isin_to_symbol, &mut name_to_symbol);
        let mf_index = load_scheme_index_from_file(&default_scheme_list_cache_path())
            .unwrap_or_default();
        Self {
            isin_to_symbol,
            name_to_symbol,
            india_ctx: default_india_symbol_context(),
            mf_index,
        }
    }

    fn with_mf_index(mf_index: SchemeIndex) -> Self {
        let mut resolver = Self::new();
        if !mf_index.is_empty() {
            resolver.mf_index = mf_index;
        }
        resolver
    }

    fn resolve(
        &self,
        symbol: Option<&str>,
        stock_name: Option<&str>,
        isin: Option<&str>,
    ) -> Result<String> {
        if let Some(isin) = isin.and_then(normalize_isin) {
            if let Some(sym) = self.isin_to_symbol.get(&isin) {
                return Ok(sym.clone());
            }
            if let Some(code) = self.mf_index.lookup_isin(&isin) {
                return Ok(stocker_mf::mf_symbol(code));
            }
        }
        if let Some(sym) = symbol.filter(|s| !s.trim().is_empty()) {
            if let Ok(resolved) = resolve_india_symbol(sym, &self.india_ctx) {
                return Ok(resolved);
            }
            if let Some(mf_sym) = self.mf_index.resolve_symbol(Some(sym), None, None) {
                return Ok(mf_sym);
            }
            return Err(Error::InvalidInput(format!(
                "could not resolve symbol for '{sym}'"
            )));
        }
        if let Some(name) = stock_name.filter(|s| !s.trim().is_empty()) {
            let key = normalize_name_key(name);
            if let Some(sym) = self.name_to_symbol.get(&key) {
                return Ok(sym.clone());
            }
            if let Some(mf_sym) = self.mf_index.resolve_symbol(None, Some(name), None) {
                return Ok(mf_sym);
            }
            return Err(Error::InvalidInput(format!(
                "could not resolve symbol for '{name}'"
            )));
        }
        Err(Error::InvalidInput("symbol, stock name, or ISIN required".into()))
    }
}

fn load_isin_index(
    path: &str,
    is_nse: bool,
    isin_to_symbol: &mut HashMap<String, String>,
    name_to_symbol: &mut HashMap<String, String>,
) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        Path::new(path).to_path_buf(),
        manifest.join("../../").join(path),
        manifest.join(path),
    ];
    let bytes = candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok());
    let bytes = match bytes {
        Some(b) => b,
        None => return,
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return;
    };
    let mut lines = text.lines();
    let header = match lines.next() {
        Some(h) => h,
        None => return,
    };
    let headers: Vec<String> = header.split(',').map(|s| s.trim().to_uppercase()).collect();
    let symbol_col = headers
        .iter()
        .position(|h| h == "SYMBOL" || h == "SECURITY ID" || h == "SECURITY_ID");
    let name_col = headers
        .iter()
        .position(|h| h == "NAME OF COMPANY" || h == "ISSUER NAME" || h == "SECURITY NAME");
    let isin_col = headers.iter().position(|h| {
        h == "ISIN NUMBER" || h == "ISIN NO" || h == "ISIN_NO" || h == "ISIN"
    });
    let (symbol_col, isin_col) = match (symbol_col, isin_col) {
        (Some(s), Some(i)) => (s, i),
        _ => return,
    };
    for line in lines {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() <= symbol_col.max(isin_col) {
            continue;
        }
        let base = cols[symbol_col].trim().to_uppercase();
        if base.is_empty() {
            continue;
        }
        let yahoo = if is_nse {
            format!("{base}.NS")
        } else {
            format!("{base}.BO")
        };
        if let Some(isin) = normalize_isin(cols[isin_col].trim()) {
            isin_to_symbol.entry(isin).or_insert_with(|| yahoo.clone());
        }
        if let Some(nc) = name_col {
            if cols.len() > nc {
                let key = normalize_name_key(cols[nc]);
                name_to_symbol.entry(key).or_insert(yahoo);
            }
        }
    }
}

fn normalize_isin(raw: &str) -> Option<String> {
    let s = raw.trim().to_uppercase();
    if s.len() == 12 && s.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(s)
    } else {
        None
    }
}

fn normalize_name_key(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace(" ltd.", " limited")
        .replace(" ltd", " limited")
        .replace('.', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_file(bytes: &[u8], filename: &str) -> Result<RawGrid> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "csv" => parse_csv(bytes),
        "xls" | "xlsx" | "xlsm" => parse_spreadsheet(bytes, &ext),
        _ => Err(Error::InvalidInput(format!(
            "unsupported file type: {ext} (use .csv, .xls, or .xlsx)"
        ))),
    }
}

fn parse_csv(bytes: &[u8]) -> Result<RawGrid> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes);
    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| Error::InvalidInput(format!("csv parse: {e}")))?;
        rows.push(record.iter().map(|s| s.trim().to_string()).collect());
    }
    Ok(RawGrid {
        rows,
        sheet_names: vec!["CSV".to_string()],
    })
}

fn parse_spreadsheet(bytes: &[u8], ext: &str) -> Result<RawGrid> {
    if let Ok(grid) = parse_with_calamine(bytes) {
        if !grid.rows.is_empty() {
            return Ok(grid);
        }
    }
    if ext == "xls" {
        if let Ok(grid) = parse_with_xlrd(bytes) {
            if !grid.rows.is_empty() {
                return Ok(grid);
            }
        }
    }
    Err(Error::InvalidInput(
        "could not parse spreadsheet; try saving as .xlsx or .csv".into(),
    ))
}

fn parse_with_xlrd(bytes: &[u8]) -> Result<RawGrid> {
    let path = std::env::temp_dir().join(format!("stocker_import_{}.xls", std::process::id()));
    std::fs::write(&path, bytes)
        .map_err(|e| Error::InvalidInput(format!("temp xls write: {e}")))?;
    let result = (|| {
        let book = xlrd::open(path.to_string_lossy().as_ref())
            .map_err(|e| Error::InvalidInput(format!("xls read: {e}")))?;
        let sheets = book.get_sheet_collection();
        let sheet_names: Vec<String> = sheets.iter().map(|s| s.get_name().to_string()).collect();
        let mut rows = Vec::new();
        for (si, sheet) in sheets.iter().enumerate() {
            if si > 0 {
                rows.push(Vec::new());
            }
            let max_row = sheet.get_highest_row() as usize;
            let max_col = sheet.get_highest_column() as usize;
            for r in 1..=max_row {
                let mut row = Vec::new();
                for c in 1..=max_col {
                    let val = sheet
                        .get_cell((c as u32, r as u32))
                        .map(|cell| cell.get_value().to_string())
                        .unwrap_or_default();
                    row.push(val);
                }
                while row.last().is_some_and(|s| s.is_empty()) {
                    row.pop();
                }
                if row.iter().any(|s| !s.is_empty()) {
                    rows.push(row);
                }
            }
        }
        Ok(RawGrid { rows, sheet_names })
    })();
    let _ = std::fs::remove_file(&path);
    result
}

fn parse_with_calamine(bytes: &[u8]) -> Result<RawGrid> {
    let cursor = std::io::Cursor::new(bytes);
    let mut workbook =
        open_workbook_auto_from_rs(cursor).map_err(|e| Error::InvalidInput(format!("xlsx: {e}")))?;
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let mut rows = Vec::new();
    for (si, sheet_name) in sheet_names.iter().enumerate() {
        if si > 0 {
            rows.push(Vec::new());
        }
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| Error::InvalidInput(format!("sheet: {e}")))?;
        for row in range.rows() {
            rows.push(row.iter().map(cell_to_string).collect());
        }
    }
    Ok(RawGrid { rows, sheet_names })
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => format_number(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => dt.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(_) => String::new(),
    }
}

fn format_number(n: f64) -> String {
    if (n - n.round()).abs() < 1e-9 {
        format!("{}", n.round() as i64)
    } else {
        format!("{n}")
    }
}

pub fn build_preview(grid: &RawGrid) -> ParsePreview {
    let suggested_header_row = detect_header_row(&grid.rows);
    let headers = grid
        .rows
        .get(suggested_header_row)
        .cloned()
        .unwrap_or_default();
    let suggested_mapping = headers.iter().map(|h| suggest_field(h)).collect();
    ParsePreview {
        grid: grid.clone(),
        suggested_header_row,
        suggested_mapping,
    }
}

fn detect_header_row(rows: &[Vec<String>]) -> usize {
    for (i, row) in rows.iter().enumerate().take(30) {
        if is_header_row(row) {
            return i;
        }
    }
    0
}

fn is_header_row(row: &[String]) -> bool {
    let joined = row.join(" ").to_lowercase();
    joined.contains("transaction date")
        && (joined.contains("transaction type")
            || joined.contains("stock")
            || joined.contains("scheme name")
            || joined.contains("scheme"))
}

fn count_header_rows(rows: &[Vec<String>]) -> usize {
    rows.iter().filter(|r| is_header_row(r)).count()
}

fn uses_multi_section_import(grid: &RawGrid) -> bool {
    grid.sheet_names.len() > 1 || count_header_rows(&grid.rows) > 1
}

fn is_preamble_row(row: &[String]) -> bool {
    if !row.iter().any(|c| !c.trim().is_empty()) {
        return true;
    }
    let joined = row.join(" ").trim().to_lowercase();
    joined.contains("total")
        || joined == "mutual funds"
        || joined.contains("stocks & etfs")
        || joined.contains("as of transaction date")
        || joined == "stocks & etfs"
}

pub fn suggest_field(header: &str) -> ImportField {
    let h = header.trim().to_lowercase();
    if h.contains("transaction date") || h == "date" || h == "trade date" {
        ImportField::TradeDate
    } else if h.contains("transaction type") || h == "type" {
        ImportField::TxnType
    } else if h.contains("stock") || h.contains("etf name") || h.contains("security name")
        || h.contains("fund") || h.contains("scheme")
    {
        ImportField::StockName
    } else if h == "symbol" || h.contains("ticker") {
        ImportField::Symbol
    } else if h.contains("isin") {
        ImportField::Isin
    } else if h.contains("balance") && (h.contains("quantity") || h.contains("unit")) {
        ImportField::EligibleQuantity
    } else if h == "units" || h.contains("quantity") {
        ImportField::Quantity
    } else if h == "nav" || h == "price" {
        ImportField::Price
    } else if h == "amount" || h.contains("gross") {
        ImportField::GrossAmount
    } else if h.contains("brokerage") {
        ImportField::Brokerage
    } else if h.contains("tax") {
        ImportField::Taxes
    } else if h.contains("net amount") || h == "net" {
        ImportField::NetAmount
    } else if h.contains("dividend") {
        ImportField::DividendPerShare
    } else if h.contains("note") {
        ImportField::Notes
    } else {
        ImportField::Skip
    }
}

pub fn preview_rows(
    portfolio_id: i64,
    header_row: usize,
    column_mapping: &[ImportField],
    grid: &RawGrid,
) -> Vec<ImportRowPreview> {
    let mf_index = load_scheme_index_from_file(&default_scheme_list_cache_path()).unwrap_or_default();
    preview_rows_with_index(portfolio_id, header_row, column_mapping, grid, &mf_index)
}

pub fn preview_rows_with_index(
    portfolio_id: i64,
    header_row: usize,
    column_mapping: &[ImportField],
    grid: &RawGrid,
    mf_index: &SchemeIndex,
) -> Vec<ImportRowPreview> {
    let resolver = SymbolResolver::with_mf_index(mf_index.clone());
    if uses_multi_section_import(grid) {
        return preview_rows_multi_section(portfolio_id, grid, &resolver);
    }
    grid.rows
        .iter()
        .enumerate()
        .skip(header_row + 1)
        .filter(|(_, row)| row.iter().any(|c| !c.trim().is_empty()))
        .map(|(row_index, row)| {
            match map_row(portfolio_id, row, column_mapping, &resolver) {
                Ok(txn) => ImportRowPreview {
                    row_index,
                    transaction: Some(txn),
                    error: None,
                },
                Err(e) => ImportRowPreview {
                    row_index,
                    transaction: None,
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}

fn preview_rows_multi_section(
    portfolio_id: i64,
    grid: &RawGrid,
    resolver: &SymbolResolver,
) -> Vec<ImportRowPreview> {
    let mut current_mapping: Vec<ImportField> = Vec::new();
    let mut results = Vec::new();
    for (row_index, row) in grid.rows.iter().enumerate() {
        if is_header_row(row) {
            current_mapping = row.iter().map(|h| suggest_field(h)).collect();
            continue;
        }
        if is_preamble_row(row) {
            continue;
        }
        if current_mapping.is_empty() {
            continue;
        }
        match map_row(portfolio_id, row, &current_mapping, resolver) {
            Ok(txn) => results.push(ImportRowPreview {
                row_index,
                transaction: Some(txn),
                error: None,
            }),
            Err(e) => results.push(ImportRowPreview {
                row_index,
                transaction: None,
                error: Some(e.to_string()),
            }),
        }
    }
    results
}

fn map_row(
    portfolio_id: i64,
    row: &[String],
    column_mapping: &[ImportField],
    resolver: &SymbolResolver,
) -> Result<NewTransaction> {
    let mut txn_type_raw = None;
    let mut trade_date_raw = None;
    let mut symbol_raw = None;
    let mut stock_name_raw = None;
    let mut isin_raw = None;
    let mut quantity_raw = None;
    let mut price_raw = None;
    let mut gross_raw = None;
    let mut brokerage_raw = None;
    let mut taxes_raw = None;
    let mut net_raw = None;
    let mut split_num_raw = None;
    let mut split_den_raw = None;
    let mut bonus_num_raw = None;
    let mut bonus_den_raw = None;
    let mut dividend_raw = None;
    let mut tds_raw = None;
    let mut eligible_raw = None;
    let mut notes_raw = None;

    for (col_idx, field) in column_mapping.iter().enumerate() {
        let val = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
        if val.trim().is_empty() {
            continue;
        }
        match field {
            ImportField::Skip => {}
            ImportField::TxnType => txn_type_raw = Some(val),
            ImportField::TradeDate => trade_date_raw = Some(val),
            ImportField::Symbol => symbol_raw = Some(val),
            ImportField::StockName => stock_name_raw = Some(val),
            ImportField::Isin => isin_raw = Some(val),
            ImportField::Quantity => quantity_raw = Some(val),
            ImportField::Price => price_raw = Some(val),
            ImportField::GrossAmount => gross_raw = Some(val),
            ImportField::Brokerage => brokerage_raw = Some(val),
            ImportField::Taxes => taxes_raw = Some(val),
            ImportField::NetAmount => net_raw = Some(val),
            ImportField::SplitRatioNum => split_num_raw = Some(val),
            ImportField::SplitRatioDen => split_den_raw = Some(val),
            ImportField::BonusRatioNum => bonus_num_raw = Some(val),
            ImportField::BonusRatioDen => bonus_den_raw = Some(val),
            ImportField::DividendPerShare => dividend_raw = Some(val),
            ImportField::Tds => tds_raw = Some(val),
            ImportField::EligibleQuantity => eligible_raw = Some(val),
            ImportField::Notes => notes_raw = Some(val),
        }
    }

    let txn_type = parse_txn_type(
        txn_type_raw.ok_or_else(|| Error::InvalidInput("transaction type is required".into()))?,
    )?;

    let trade_date = parse_date(
        trade_date_raw.ok_or_else(|| Error::InvalidInput("trade date is required".into()))?,
    )?;

    if is_summary_row(stock_name_raw, txn_type_raw) {
        return Err(Error::InvalidInput("summary row".into()));
    }

    let symbol = resolver.resolve(symbol_raw, stock_name_raw, isin_raw)?;

    let quantity = quantity_raw.and_then(parse_number);
    let price = price_raw.and_then(parse_number);
    let gross_amount = gross_raw.and_then(parse_number);
    let brokerage = brokerage_raw.and_then(parse_number);
    let taxes = taxes_raw.and_then(parse_number);
    let net_amount = net_raw.and_then(parse_number);

    let mut input = NewTransaction {
        portfolio_id,
        txn_type,
        trade_date,
        symbol: Some(symbol),
        quantity,
        price,
        gross_amount,
        brokerage,
        taxes,
        net_amount,
        split_ratio_num: split_num_raw.and_then(parse_number),
        split_ratio_den: split_den_raw.and_then(parse_number),
        bonus_ratio_num: bonus_num_raw.and_then(parse_number),
        bonus_ratio_den: bonus_den_raw.and_then(parse_number),
        dividend_per_share: dividend_raw.and_then(parse_number),
        tds: tds_raw.and_then(parse_number),
        eligible_quantity: eligible_raw.and_then(parse_number),
        notes: notes_raw.map(|s| s.to_string()),
    };

    finalize_import_transaction(
        &mut input,
        txn_type_raw,
        quantity_raw,
        price_raw,
        notes_raw.as_deref(),
    )?;
    crate::transactions::validate_new(&input)?;
    Ok(input)
}

fn abs_opt(v: Option<f64>) -> Option<f64> {
    v.map(f64::abs).filter(|x| *x > 0.0)
}

fn positive_qty(qty: Option<f64>) -> Option<f64> {
    qty.map(f64::abs).filter(|q| *q > 0.0)
}

/// Prefer broker Units × NAV; only derive missing fields from Amount when needed.
fn finalize_units_and_nav_trade(input: &mut NewTransaction) -> Result<()> {
    let qty = positive_qty(input.quantity);
    let price = abs_opt(input.price);
    let amount = abs_opt(input.gross_amount).or_else(|| abs_opt(input.net_amount));

    if let (Some(q), Some(p)) = (qty, price) {
        input.quantity = Some(q);
        input.price = Some(p);
        let gross = q * p;
        input.gross_amount = Some(gross);
        input.net_amount = Some(gross);
        return Ok(());
    }

    let mut qty = qty;
    if qty.is_none() {
        if let (Some(a), Some(p)) = (amount, price) {
            qty = Some(a / p);
        }
    }

    let qty = qty.ok_or_else(|| {
        Error::InvalidInput(format!(
            "{} requires quantity (or amount and price to derive it)",
            input.txn_type.as_str()
        ))
    })?;

    input.quantity = Some(qty);
    input.price = price.or_else(|| amount.map(|a| a / qty));
    let gross = if let Some(p) = input.price {
        qty * p
    } else {
        amount.unwrap_or(0.0)
    };
    input.gross_amount = Some(gross);
    input.net_amount = Some(gross);
    Ok(())
}

/// Parse `"5:1"`, `"1/5"`, or a plain number as `(num, den)`.
pub fn parse_ratio_pair(raw: &str) -> Option<(f64, f64)> {
    let s = raw.trim();
    if s.is_empty() || s == "--" || s == "-" || s == "—" {
        return None;
    }
    for sep in [':', '/'] {
        if let Some((a, b)) = s.split_once(sep) {
            let num = parse_number(a)?;
            let den = parse_number(b)?;
            if num > 0.0 && den > 0.0 {
                return Some((num, den));
            }
        }
    }
    parse_number(s)
        .filter(|n| *n > 0.0)
        .map(|n| (n, 1.0))
}

fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}

fn simplify_ratio(num: f64, den: f64) -> (f64, f64) {
    let n = num.round() as i64;
    let d = den.round() as i64;
    if n > 0 && d > 0 && (num - n as f64).abs() < 1e-6 && (den - d as f64).abs() < 1e-6 {
        let g = gcd_i64(n, d);
        return ((n / g) as f64, (d / g) as f64);
    }
    (num, den)
}

fn infer_split_ratio(
    input: &mut NewTransaction,
    txn_type_raw: Option<&str>,
    quantity_raw: Option<&str>,
    price_raw: Option<&str>,
    notes_raw: Option<&str>,
) -> Result<()> {
    if input.split_ratio_num.filter(|n| *n > 0.0).is_none() {
        let is_stock_split = txn_type_raw
            .map(|s| s.trim().to_lowercase().contains("stock split"))
            .unwrap_or(false);

        let inferred = if is_stock_split {
            match (
                input.quantity.filter(|q| *q > 0.0),
                input.eligible_quantity.filter(|q| *q > 0.0),
            ) {
                (Some(pre), Some(post)) if post > pre => Some(simplify_ratio(post, pre)),
                _ => None,
            }
        } else {
            None
        }
        .or_else(|| quantity_raw.and_then(parse_ratio_pair))
        .or_else(|| price_raw.and_then(parse_ratio_pair))
        .or_else(|| notes_raw.and_then(parse_ratio_pair))
        .or_else(|| {
            input
                .quantity
                .filter(|q| *q > 0.0)
                .map(|q| (q, 1.0))
        })
        .or_else(|| {
            input
                .price
                .filter(|p| *p > 0.0)
                .map(|p| (p, 1.0))
        });
        match inferred {
            Some((num, den)) => {
                input.split_ratio_num = Some(num);
                input.split_ratio_den = Some(den);
            }
            None => {
                return Err(Error::InvalidInput(
                    "split requires split ratio columns".into(),
                ));
            }
        }
    } else if input.split_ratio_den.filter(|d| *d > 0.0).is_none() {
        input.split_ratio_den = Some(1.0);
    }

    input.quantity = None;
    input.price = None;
    input.gross_amount = None;
    input.net_amount = None;
    input.brokerage = None;
    input.taxes = None;
    Ok(())
}

/// Normalize broker-export rows: prefer Units × NAV, abs redemption amounts, infer missing qty.
fn finalize_import_transaction(
    input: &mut NewTransaction,
    txn_type_raw: Option<&str>,
    quantity_raw: Option<&str>,
    price_raw: Option<&str>,
    notes_raw: Option<&str>,
) -> Result<()> {
    match input.txn_type {
        TransactionType::OpeningBalance
        | TransactionType::Buy
        | TransactionType::MergerInvestment
        | TransactionType::DemergerInvestment
        | TransactionType::Rights
        | TransactionType::Sell
        | TransactionType::MergerRedemption
        | TransactionType::DemergerRedemption => finalize_units_and_nav_trade(input)?,
        TransactionType::Dividend => {
            input.quantity = None;
            if input.dividend_per_share.filter(|d| *d > 0.0).is_none() {
                input.dividend_per_share = abs_opt(input.price);
            }
            let amount = abs_opt(input.gross_amount).or_else(|| abs_opt(input.net_amount));
            if let Some(amt) = amount {
                input.gross_amount = Some(amt);
                input.net_amount = Some(amt);
            } else if let (Some(dps), Some(eq)) =
                (input.dividend_per_share, input.eligible_quantity.filter(|q| *q > 0.0))
            {
                let gross = dps * eq;
                input.gross_amount = Some(gross);
                input.net_amount = Some(gross);
            }
            input.price = None;
        }
        TransactionType::Bonus => {
            if input.bonus_ratio_num.filter(|n| *n > 0.0).is_none() {
                let qty = input
                    .quantity
                    .filter(|q| *q > 0.0)
                    .or_else(|| input.eligible_quantity.filter(|q| *q > 0.0));
                if let Some(qty) = qty {
                    // Broker bonus lines list shares received — record as zero-cost buy.
                    input.txn_type = TransactionType::Buy;
                    input.quantity = Some(qty);
                    input.price = Some(0.0);
                    input.gross_amount = Some(0.0);
                    input.net_amount = Some(0.0);
                } else {
                    return Err(Error::InvalidInput(
                        "bonus requires quantity or bonus ratio columns".into(),
                    ));
                }
            }
        }
        TransactionType::Split => {
            infer_split_ratio(input, txn_type_raw, quantity_raw, price_raw, notes_raw)?;
        }
        TransactionType::Sip => {
            let amount = abs_opt(input.gross_amount).or_else(|| abs_opt(input.net_amount));
            if let Some(amt) = amount {
                input.gross_amount = Some(amt);
                input.net_amount = Some(amt);
            }
            let is_mf = input
                .symbol
                .as_deref()
                .is_some_and(is_mutual_fund_symbol);
            // Broker MF exports: SIP Investment rows include allotted units × NAV — record as buys.
            if is_mf {
                if let (Some(qty), Some(price)) = (positive_qty(input.quantity), abs_opt(input.price))
                {
                    input.txn_type = TransactionType::Buy;
                    input.quantity = Some(qty);
                    input.price = Some(price);
                    let gross = amount.unwrap_or(qty * price);
                    input.gross_amount = Some(gross);
                    input.net_amount = Some(gross);
                    input.eligible_quantity = None;
                }
            } else if let (Some(qty), Some(price)) =
                (positive_qty(input.quantity), abs_opt(input.price))
            {
                input.txn_type = TransactionType::Buy;
                input.quantity = Some(qty);
                input.price = Some(price);
                let gross = amount.unwrap_or(qty * price);
                input.gross_amount = Some(gross);
                input.net_amount = Some(gross);
                input.eligible_quantity = None;
            }
        }
    }
    Ok(())
}

fn is_summary_row(stock_name: Option<&str>, txn_type: Option<&str>) -> bool {
    let name = stock_name.unwrap_or("").to_lowercase();
    let typ = txn_type.unwrap_or("").to_lowercase();
    name.contains("total") || typ.contains("total")
}

pub fn parse_txn_type(raw: &str) -> Result<TransactionType> {
    let key = raw.trim().to_lowercase();
    let mapped = match key.as_str() {
        "investment in stock" | "buy" | "purchase" | "investment in fund" => TransactionType::Buy,
        "merger investment" => TransactionType::MergerInvestment,
        "demerger investment" => TransactionType::DemergerInvestment,
        "merger redemption" => TransactionType::MergerRedemption,
        "demerger redemption" => TransactionType::DemergerRedemption,
        "sell" | "sale" | "sell/redemption" | "redemption" => TransactionType::Sell,
        "dividend" => TransactionType::Dividend,
        "bonus" => TransactionType::Bonus,
        "rights" | "rights issue" => TransactionType::Rights,
        "split" | "splits" | "stock split" => TransactionType::Split,
        "opening balance" => TransactionType::OpeningBalance,
        "sip investment" | "sip" => TransactionType::Sip,
        _ => {
            return Err(Error::InvalidInput(format!(
                "unknown transaction type: {raw}"
            )));
        }
    };
    Ok(mapped)
}

pub fn parse_date(raw: &str) -> Result<String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(Error::InvalidInput("empty date".into()));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%d/%m/%Y") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%d-%b-%y") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%d-%b-%Y") {
        return Ok(d.format("%Y-%m-%d").to_string());
    }
    Err(Error::InvalidInput(format!("unrecognized date: {raw}")))
}

pub fn parse_number(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if s.is_empty() || s == "--" || s == "-" || s == "—" {
        return None;
    }
    let cleaned: String = s
        .chars()
        .filter(|c| *c != '₹' && *c != ',' && *c != '%' && *c != '#')
        .collect::<String>()
        .trim()
        .to_string();
    if cleaned.is_empty() || cleaned == "--" {
        return None;
    }
    cleaned.parse().ok()
}

/// Import order: earliest date first; on the same date, buys before sells so FIFO `id`
/// order matches chronological intent when the broker file lists rows out of order.
fn import_txn_sort_key(txn_type: TransactionType) -> u8 {
    match txn_type {
        TransactionType::OpeningBalance => 0,
        TransactionType::Buy
        | TransactionType::MergerInvestment
        | TransactionType::DemergerInvestment
        | TransactionType::Rights
        | TransactionType::Sip => 1,
        TransactionType::Split | TransactionType::Bonus => 2,
        TransactionType::Dividend => 3,
        TransactionType::Sell
        | TransactionType::MergerRedemption
        | TransactionType::DemergerRedemption => 4,
    }
}

pub fn sort_transactions_for_import(txns: &mut [NewTransaction]) {
    txns.sort_by(|a, b| {
        a.trade_date
            .cmp(&b.trade_date)
            .then_with(|| {
                import_txn_sort_key(a.txn_type).cmp(&import_txn_sort_key(b.txn_type))
            })
    });
}

pub async fn bulk_import(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    mut transactions: Vec<NewTransaction>,
) -> Result<ImportResult> {
    bulk_import_with_mf(pool, user_id, portfolio_id, transactions, None).await
}

pub async fn bulk_import_with_mf(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    mut transactions: Vec<NewTransaction>,
    mf: Option<Arc<MfService>>,
) -> Result<ImportResult> {
    let _ = portfolios::get(pool, user_id, portfolio_id).await?;
    sort_transactions_for_import(&mut transactions);
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();

    for (idx, input) in transactions.into_iter().enumerate() {
        match bulk_insert_one(pool, user_id, &input).await {
            Ok(()) => imported += 1,
            Err(e) => {
                skipped += 1;
                errors.push(ImportRowError {
                    row_index: idx,
                    message: e.to_string(),
                });
            }
        }
    }

    if imported > 0 {
        if let Some(mf) = mf {
            let _ = crate::sip_refresh::refresh_sip_transactions(
                pool,
                Some(mf),
                user_id,
                portfolio_id,
            )
            .await;
        }
        rebuild::rebuild(pool, portfolio_id).await?;
    }

    Ok(ImportResult {
        imported,
        skipped,
        errors,
    })
}

async fn bulk_insert_one(pool: &SqlitePool, user_id: i64, input: &NewTransaction) -> Result<()> {
    crate::transactions::validate_new(input)?;
    let now = chrono::Utc::now().timestamp();
    let corp_key = crate::transactions::corporate_action_key(input);

    sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'import', ?, ?, ?)",
    )
    .bind(user_id)
    .bind(input.portfolio_id)
    .bind(input.txn_type.as_str())
    .bind(&input.trade_date)
    .bind(input.symbol.as_deref())
    .bind(input.quantity)
    .bind(input.price)
    .bind(input.gross_amount)
    .bind(input.brokerage)
    .bind(input.taxes)
    .bind(input.net_amount)
    .bind(input.split_ratio_num)
    .bind(input.split_ratio_den)
    .bind(input.bonus_ratio_num)
    .bind(input.bonus_ratio_den)
    .bind(input.dividend_per_share)
    .bind(input.tds)
    .bind(input.eligible_quantity)
    .bind(input.notes.as_deref())
    .bind(corp_key.as_deref())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() {
                return Error::Conflict("corporate action already applied".into());
            }
        }
        Error::from(e)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_transactions_for_import_orders_by_date_then_buys_before_sells() {
        fn txn(trade_date: &str, txn_type: TransactionType) -> NewTransaction {
            NewTransaction {
                portfolio_id: 1,
                txn_type,
                trade_date: trade_date.into(),
                symbol: Some("MF:1".into()),
                quantity: Some(1.0),
                price: Some(1.0),
                gross_amount: Some(1.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(1.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            }
        }

        let mut txns = vec![
            txn("2024-06-01", TransactionType::Sell),
            txn("2024-01-01", TransactionType::Buy),
            txn("2024-01-01", TransactionType::Sell),
        ];
        sort_transactions_for_import(&mut txns);
        assert_eq!(txns[0].trade_date, "2024-01-01");
        assert_eq!(txns[0].txn_type, TransactionType::Buy);
        assert_eq!(txns[1].trade_date, "2024-01-01");
        assert_eq!(txns[1].txn_type, TransactionType::Sell);
        assert_eq!(txns[2].trade_date, "2024-06-01");
        assert_eq!(txns[2].txn_type, TransactionType::Sell);
    }

    #[test]
    fn parse_txn_type_aliases() {
        assert_eq!(parse_txn_type("Investment in stock").unwrap(), TransactionType::Buy);
        assert_eq!(
            parse_txn_type("Demerger Redemption").unwrap(),
            TransactionType::DemergerRedemption
        );
        assert_eq!(parse_txn_type("Splits").unwrap(), TransactionType::Split);
        assert_eq!(parse_txn_type("SIP Investment").unwrap(), TransactionType::Sip);
        assert_eq!(
            parse_txn_type("Sell/Redemption").unwrap(),
            TransactionType::Sell
        );
    }

    #[test]
    fn parse_date_formats() {
        assert_eq!(parse_date("05-Sep-19").unwrap(), "2019-09-05");
        assert_eq!(parse_date("2024-01-15").unwrap(), "2024-01-15");
    }

    #[test]
    fn parse_number_strips_currency() {
        assert_eq!(parse_number("₹1,234.50"), Some(1234.5));
        assert_eq!(parse_number("-₹514"), Some(-514.0));
        assert_eq!(parse_number("--"), None);
    }

    #[test]
    fn parse_ratio_pair_formats() {
        assert_eq!(parse_ratio_pair("5:1"), Some((5.0, 1.0)));
        assert_eq!(parse_ratio_pair("1/5"), Some((1.0, 5.0)));
        assert_eq!(parse_ratio_pair("5"), Some((5.0, 1.0)));
    }

    #[test]
    fn sip_with_units_converts_to_buy() {
        let mut input = NewTransaction {
            portfolio_id: 1,
            txn_type: TransactionType::Sip,
            trade_date: "2025-02-21".into(),
            symbol: Some("MF:145552".into()),
            quantity: Some(115.974),
            price: Some(43.1111),
            gross_amount: Some(5000.0),
            brokerage: None,
            taxes: None,
            net_amount: Some(5000.0),
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: Some(243258.22),
            notes: None,
        };
        finalize_import_transaction(&mut input, Some("SIP Investment"), Some("115.974"), Some("43.1111"), None)
            .unwrap();
        assert_eq!(input.txn_type, TransactionType::Buy);
        assert_eq!(input.quantity, Some(115.974));
        assert_eq!(input.price, Some(43.1111));
        assert_eq!(input.gross_amount, Some(5000.0));
        assert!(input.eligible_quantity.is_none());
    }

    #[test]
    fn split_import_infers_ratio_from_quantity() {
        let mut input = NewTransaction {
            portfolio_id: 1,
            txn_type: TransactionType::Split,
            trade_date: "2024-07-01".into(),
            symbol: Some("ITC.NS".into()),
            quantity: Some(5.0),
            price: None,
            gross_amount: None,
            brokerage: None,
            taxes: None,
            net_amount: None,
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: Some(500.0),
            notes: None,
        };
        finalize_import_transaction(&mut input, Some("Splits"), Some("5"), None, None).unwrap();
        assert_eq!(input.split_ratio_num, Some(5.0));
        assert_eq!(input.split_ratio_den, Some(1.0));
        assert!(input.quantity.is_none());
    }

    #[test]
    fn stock_split_infers_ratio_from_balance_over_quantity() {
        let mut input = NewTransaction {
            portfolio_id: 1,
            txn_type: TransactionType::Split,
            trade_date: "2024-05-24".into(),
            symbol: Some("BDL.NS".into()),
            quantity: Some(100.0),
            price: Some(1523.05),
            gross_amount: None,
            brokerage: None,
            taxes: None,
            net_amount: None,
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: Some(200.0),
            notes: None,
        };
        finalize_import_transaction(
            &mut input,
            Some("Stock Split"),
            Some("100"),
            Some("1523.05"),
            None,
        )
        .unwrap();
        assert_eq!(input.split_ratio_num, Some(2.0));
        assert_eq!(input.split_ratio_den, Some(1.0));
        assert!(input.quantity.is_none());
        assert!(input.price.is_none());
    }

    #[test]
    fn regret_xls_stock_splits_use_balance_ratio() {
        let path = "/Users/devendermishra/Downloads/Transaction History_Regret_All-time.xls";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let bytes = std::fs::read(path).unwrap();
        let grid = parse_file(&bytes, "regret.xls").unwrap();
        let preview = build_preview(&grid);
        let rows = preview_rows(
            1,
            preview.suggested_header_row,
            &preview.suggested_mapping,
            &preview.grid,
        );
        let splits: Vec<_> = rows
            .iter()
            .filter_map(|r| r.transaction.as_ref())
            .filter(|t| t.txn_type == TransactionType::Split)
            .collect();
        assert_eq!(splits.len(), 2);
        for split in splits {
            let num = split.split_ratio_num.unwrap();
            let den = split.split_ratio_den.unwrap();
            let factor = num / den;
            assert!(
                (1.9..=2.1).contains(&factor),
                "expected ~2:1 split, got {num}/{den}"
            );
        }
    }

    #[test]
    fn mf_redemption_prefers_units_over_amount_div_nav() {
        let mut input = NewTransaction {
            portfolio_id: 1,
            txn_type: TransactionType::Sell,
            trade_date: "2024-07-15".into(),
            symbol: Some("MF:112277".into()),
            quantity: Some(-944.461),
            price: Some(59.43),
            gross_amount: Some(-56129.32),
            brokerage: None,
            taxes: None,
            net_amount: None,
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
        };
        finalize_import_transaction(&mut input, None, None, None, None).unwrap();
        assert_eq!(input.quantity, Some(944.461));
        assert_eq!(input.price, Some(59.43));
        assert_eq!(input.gross_amount, Some(944.461 * 59.43));
    }

    #[test]
    fn suggest_field_maps_mf_export_headers() {
        assert_eq!(suggest_field("Units"), ImportField::Quantity);
        assert_eq!(suggest_field("NAV"), ImportField::Price);
        assert_eq!(suggest_field("Balance Units"), ImportField::EligibleQuantity);
    }

    #[test]
    fn detect_header_row_from_fixture_or_csv() {
        let bytes = include_bytes!("../tests/fixtures/transaction_history_sample.xls");
        let grid = match parse_file(bytes, "transaction_history_sample.xls") {
            Ok(g) => g,
            Err(_) => {
                let csv = include_str!("../tests/fixtures/transaction_history_sample.csv");
                parse_file(csv.as_bytes(), "transaction_history_sample.csv").unwrap()
            }
        };
        let preview = build_preview(&grid);
        assert!(preview.suggested_header_row > 0);
        let headers = &grid.rows[preview.suggested_header_row];
        assert!(headers.iter().any(|h| h.contains("Transaction Date")));
        assert!(headers.iter().any(|h| h.contains("Transaction Type")));
    }

    #[test]
    fn isin_resolves_itc() {
        let resolver = SymbolResolver::new();
        let sym = resolver
            .resolve(None, None, Some("INE154A01025"))
            .unwrap();
        assert_eq!(sym, "ITC.NS");
    }

    #[test]
    fn csv_fixture_maps_rows() {
        let csv = include_str!("../tests/fixtures/transaction_history_sample.csv");
        let grid = parse_file(csv.as_bytes(), "sample.csv").unwrap();
        let preview = build_preview(&grid);
        let rows = preview_rows(
            1,
            preview.suggested_header_row,
            &preview.suggested_mapping,
            &preview.grid,
        );
        let valid: Vec<_> = rows.iter().filter(|r| r.transaction.is_some()).collect();
        assert!(!valid.is_empty());
        let buy = valid
            .iter()
            .find(|r| {
                r.transaction
                    .as_ref()
                    .map(|t| t.txn_type == TransactionType::Buy)
                    .unwrap_or(false)
            })
            .expect("buy row");
        assert_eq!(buy.transaction.as_ref().unwrap().trade_date, "2019-09-05");
        assert_eq!(buy.transaction.as_ref().unwrap().gross_amount, Some(1000.0));
    }

    #[test]
    fn csv_fixture_normalizes_broker_rows() {
        let csv = include_str!("../tests/fixtures/transaction_history_sample.csv");
        let grid = parse_file(csv.as_bytes(), "sample.csv").unwrap();
        let preview = build_preview(&grid);
        let rows = preview_rows(
            1,
            preview.suggested_header_row,
            &preview.suggested_mapping,
            &preview.grid,
        );
        let valid: Vec<_> = rows
            .iter()
            .filter_map(|r| r.transaction.as_ref())
            .collect();
        assert_eq!(valid.len(), 6);

        let split = valid
            .iter()
            .find(|t| t.trade_date == "2024-07-01")
            .expect("split row");
        assert_eq!(split.txn_type, TransactionType::Split);
        assert_eq!(split.split_ratio_num, Some(5.0));
        assert_eq!(split.split_ratio_den, Some(1.0));
        assert!(split.quantity.is_none());

        let bonus = valid
            .iter()
            .find(|t| t.trade_date == "2025-05-23")
            .expect("bonus row");
        assert_eq!(bonus.txn_type, TransactionType::Buy);
        assert_eq!(bonus.quantity, Some(1.0));
        assert_eq!(bonus.net_amount, Some(0.0));

        let dividend = valid
            .iter()
            .find(|t| t.trade_date == "2025-05-14")
            .expect("dividend row");
        assert_eq!(dividend.txn_type, TransactionType::Dividend);
        assert_eq!(dividend.gross_amount, Some(700.0));
        assert!(dividend.quantity.is_none());

        let demerger_derived = valid
            .iter()
            .find(|t| t.trade_date == "2025-01-06")
            .expect("demerger amount row");
        assert_eq!(demerger_derived.txn_type, TransactionType::DemergerRedemption);
        assert!(demerger_derived.quantity.unwrap() > 1.0);
        assert_eq!(demerger_derived.gross_amount, Some(514.0));

        let demerger_qty = valid
            .iter()
            .find(|t| t.trade_date == "2024-06-04")
            .expect("demerger qty row");
        assert_eq!(demerger_qty.quantity, Some(10.0));
    }

    #[test]
    fn mf_name_resolves_to_mf_symbol() {
        use stocker_mf::{save_scheme_list_cache, SchemeIndex, SchemeListEntry};

        let entries = vec![SchemeListEntry {
            scheme_code: 141957,
            scheme_name: "BHARAT 22 ETF".into(),
            isin_growth: Some("INF109KB15Y7".into()),
            isin_div_reinvestment: None,
        }];
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("mf_schemes_list.json");
        save_scheme_list_cache(&cache, &entries).unwrap();
        std::env::set_var("STOCKER_MF_SCHEMES_CACHE_PATH", cache.to_str().unwrap());

        let idx = SchemeIndex::from_entries(entries);
        let resolver = SymbolResolver::with_mf_index(idx);
        let sym = resolver
            .resolve(None, Some("Bharat 22 ETF"), None)
            .unwrap();
        assert_eq!(sym, "MF:141957");

        let by_isin = resolver
            .resolve(None, None, Some("INF109KB15Y7"))
            .unwrap();
        assert_eq!(by_isin, "MF:141957");

        std::env::remove_var("STOCKER_MF_SCHEMES_CACHE_PATH");
    }

    #[test]
    fn komal_xls_imports_both_mf_and_stock_sheets() {
        let path = "/Users/devendermishra/Downloads/Transaction History_komal_All-time.xls";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let cache =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mf_schemes_list.json");
        std::env::set_var("STOCKER_MF_SCHEMES_CACHE_PATH", cache.to_str().unwrap());

        let bytes = std::fs::read(path).unwrap();
        let grid = parse_file(&bytes, "komal.xls").unwrap();
        assert_eq!(grid.sheet_names.len(), 2);
        assert!(uses_multi_section_import(&grid));

        let preview = build_preview(&grid);
        let rows = preview_rows(
            1,
            preview.suggested_header_row,
            &preview.suggested_mapping,
            &preview.grid,
        );
        let valid: Vec<_> = rows
            .iter()
            .filter_map(|r| r.transaction.as_ref())
            .collect();
        let mf = valid
            .iter()
            .filter(|t| t.symbol.as_deref().map(is_mutual_fund_symbol).unwrap_or(false))
            .count();
        let stocks = valid
            .iter()
            .filter(|t| {
                t.symbol
                    .as_deref()
                    .map(|s| !is_mutual_fund_symbol(s))
                    .unwrap_or(false)
            })
            .count();
        assert!(mf > 100, "expected MF transactions from first sheet");
        assert!(stocks > 0, "expected stock transactions from second sheet");
        assert!(
            valid
                .iter()
                .filter(|t| t.symbol.as_deref() == Some("MF:145552"))
                .all(|t| t.txn_type == TransactionType::Buy || t.txn_type == TransactionType::Sell),
            "MF SIP lines should import as buys"
        );

        std::env::remove_var("STOCKER_MF_SCHEMES_CACHE_PATH");
    }

    #[test]
    fn komal_xls_header_and_sip_types() {
        let path = "/Users/devendermishra/Downloads/Transaction History_Komal_All-time.xls";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let cache =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mf_schemes_list.json");
        std::env::set_var("STOCKER_MF_SCHEMES_CACHE_PATH", cache.to_str().unwrap());

        let bytes = std::fs::read(path).unwrap();
        let grid = parse_file(&bytes, "komal.xls").unwrap();
        let preview = build_preview(&grid);
        let rows = preview_rows(
            1,
            preview.suggested_header_row,
            &preview.suggested_mapping,
            &preview.grid,
        );
        let mut sip = 0usize;
        let mut buy = 0usize;
        let mut sell = 0usize;
        for r in &rows {
            if let Some(t) = &r.transaction {
                if t.symbol.as_deref() != Some("MF:145552") {
                    continue;
                }
                match t.txn_type {
                    TransactionType::Sip => sip += 1,
                    TransactionType::Buy => buy += 1,
                    TransactionType::Sell => sell += 1,
                    _ => {}
                }
            }
        }
        assert_eq!(sell, 1);
        assert!(buy > 50, "SIP rows should become buys when units+nav present");
        assert_eq!(sip, 0, "no SIP rows should remain for allotted MF lines");

        std::env::remove_var("STOCKER_MF_SCHEMES_CACHE_PATH");
    }

    #[tokio::test]
    async fn komal_mf_import_rebuilds_after_full_redemption() {
        use crate::auth::ensure_local_user;
        use crate::db;
        use crate::engine::rebuild;
        use crate::models::NewPortfolio;
        use crate::portfolios;
        use crate::transactions;

        let path = "/Users/devendermishra/Downloads/Transaction History_komal_All-time.xls";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let cache =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../mf_schemes_list.json");
        std::env::set_var("STOCKER_MF_SCHEMES_CACHE_PATH", cache.to_str().unwrap());

        let bytes = std::fs::read(path).unwrap();
        let grid = parse_file(&bytes, "komal.xls").unwrap();
        let preview = build_preview(&grid);
        let rows = preview_rows(
            1,
            preview.suggested_header_row,
            &preview.suggested_mapping,
            &preview.grid,
        );
        let mut txns: Vec<NewTransaction> = rows
            .into_iter()
            .filter_map(|r| r.transaction)
            .filter(|t| t.symbol.as_deref() == Some("MF:145552"))
            .collect();
        assert!(!txns.is_empty());
        sort_transactions_for_import(&mut txns);
        assert!(txns.iter().any(|t| t.txn_type == TransactionType::Sell));
        assert!(txns.iter().any(|t| t.txn_type == TransactionType::Buy));

        let pool = db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "Komal".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        for mut t in txns {
            t.portfolio_id = portfolio.id;
            transactions::create(&pool, user.id, &t).await.unwrap();
        }

        rebuild::rebuild(&pool, portfolio.id).await.unwrap();
        std::env::remove_var("STOCKER_MF_SCHEMES_CACHE_PATH");
    }
}
