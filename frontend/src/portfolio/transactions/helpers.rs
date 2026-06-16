use dioxus::prelude::*;

use crate::portfolio_api::{MfSearchHit, NewTransaction, Transaction, TransactionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Stock,
    MutualFund,
}

pub struct FormInitialState {
    pub txn_type: TransactionType,
    pub trade_date: String,
    pub asset_kind: AssetKind,
    pub symbol: String,
    pub mf_query: String,
    pub selected_mf: Option<MfSearchHit>,
    pub quantity: String,
    pub price: String,
    pub net_amount: String,
    pub split_num: String,
    pub split_den: String,
    pub bonus_num: String,
    pub bonus_den: String,
    pub dividend_per_share: String,
}

pub fn form_initial_state(txn: &Transaction) -> FormInitialState {
    let sym = txn.symbol.clone().unwrap_or_default();
    let is_mf = sym.starts_with("MF:");
    FormInitialState {
        txn_type: txn.txn_type.clone(),
        trade_date: txn.trade_date.clone(),
        asset_kind: if is_mf {
            AssetKind::MutualFund
        } else {
            AssetKind::Stock
        },
        symbol: if is_mf { String::new() } else { sym.clone() },
        mf_query: if is_mf { sym } else { String::new() },
        selected_mf: None,
        quantity: fmt_opt_f64(txn.quantity),
        price: fmt_opt_f64(txn.price),
        net_amount: fmt_opt_f64(txn.net_amount),
        split_num: fmt_opt_f64_or(txn.split_ratio_num, "5"),
        split_den: fmt_opt_f64_or(txn.split_ratio_den, "1"),
        bonus_num: fmt_opt_f64_or(txn.bonus_ratio_num, "1"),
        bonus_den: fmt_opt_f64_or(txn.bonus_ratio_den, "1"),
        dividend_per_share: fmt_opt_f64(txn.dividend_per_share),
    }
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn fmt_opt_f64_or(v: Option<f64>, default: &str) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| default.to_string())
}

pub fn parse_f64(s: &str) -> Option<f64> {
    if s.trim().is_empty() {
        None
    } else {
        s.trim().parse().ok()
    }
}

fn trade_amount_from_qty_price(quantity: &str, price: &str) -> Option<f64> {
    let qty = parse_f64(quantity)?;
    let pr = parse_f64(price)?;
    if qty > 0.0 && pr > 0.0 {
        Some(qty * pr)
    } else {
        None
    }
}

fn format_trade_amount(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{:.0}", v)
    } else {
        format!("{:.4}", v)
    }
}

pub fn maybe_fill_net_amount(quantity: &str, price: &str, net_amount: &mut Signal<String>) {
    if !net_amount().trim().is_empty() {
        return;
    }
    if let Some(amount) = trade_amount_from_qty_price(quantity, price) {
        net_amount.set(format_trade_amount(amount));
    }
}

pub fn fill_net_amount_string(quantity: &str, price: &str, net_amount: &mut String) {
    if !net_amount.trim().is_empty() {
        return;
    }
    if let Some(amount) = trade_amount_from_qty_price(quantity, price) {
        *net_amount = format_trade_amount(amount);
    }
}

pub fn build_txn(
    portfolio_id: i64,
    txn_type: TransactionType,
    trade_date: String,
    symbol: String,
    quantity: String,
    price: String,
    net_amount: String,
    split_num: String,
    split_den: String,
    bonus_num: String,
    bonus_den: String,
    dividend_per_share: String,
) -> NewTransaction {
    let sym = if symbol.trim().is_empty() {
        None
    } else {
        Some(symbol.trim().to_string())
    };
    let qty = parse_f64(&quantity);
    let pr = parse_f64(&price);
    let net = parse_f64(&net_amount).or_else(|| trade_amount_from_qty_price(&quantity, &price));
    NewTransaction {
        portfolio_id,
        txn_type,
        trade_date,
        symbol: sym,
        quantity: qty,
        price: pr,
        gross_amount: net,
        brokerage: None,
        taxes: None,
        net_amount: net,
        split_ratio_num: parse_f64(&split_num),
        split_ratio_den: parse_f64(&split_den),
        bonus_ratio_num: parse_f64(&bonus_num),
        bonus_ratio_den: parse_f64(&bonus_den),
        dividend_per_share: parse_f64(&dividend_per_share),
        tds: None,
        eligible_quantity: qty,
        notes: None,
    }
}
