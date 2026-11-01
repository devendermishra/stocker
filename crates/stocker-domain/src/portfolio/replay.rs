//! FIFO replay of portfolio transactions. Persistence stays in `stocker-portfolio`.

use std::collections::HashMap;

use thiserror::Error;

use super::models::{Transaction, TransactionType};

#[derive(Error, Debug)]
#[error("{0}")]
pub struct LedgerError(pub String);

type Result<T> = std::result::Result<T, LedgerError>;

#[derive(Debug, Clone)]
pub struct FifoReplay {
    /// Lots still open after the replay, in first-seen symbol order and then acquisition order.
    pub open_lots: Vec<(String, Lot)>,
    pub matches: Vec<PendingMatch>,
    pub symbols: HashMap<String, SymbolStats>,
}
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SymbolStats {
    pub quantity: f64,
    pub total_cost: f64,
    pub realized_gain: f64,
    pub dividend_received: f64,
    pub last_transaction_date: Option<String>,
}


#[derive(Debug, Clone)]
pub struct Lot {
    pub source_txn_id: i64,
    pub acquired_date: String,
    /// Share count when this lot was opened, after splits and bonus ratios, before sells.
    pub opened_qty: f64,
    pub remaining_qty: f64,
    pub total_cost: f64,
}

impl Lot {
    pub fn cost_per_share(&self) -> f64 {
        if self.remaining_qty <= 0.0 {
            0.0
        } else {
            self.total_cost / self.remaining_qty
        }
    }
}

#[derive(Debug, Default)]
struct SymbolState {
    lots: Vec<Lot>,
    realized_gain: f64,
    dividend_received: f64,
    last_transaction_date: Option<String>,
    applied_corp_actions: HashMap<String, ()>,
}

impl SymbolState {
    fn total_qty(&self) -> f64 {
        self.lots.iter().map(|l| l.remaining_qty).sum()
    }

    fn total_cost(&self) -> f64 {
        self.lots.iter().map(|l| l.total_cost).sum()
    }

    fn to_stats(&self) -> SymbolStats {
        SymbolStats {
            quantity: self.total_qty(),
            total_cost: self.total_cost(),
            realized_gain: self.realized_gain,
            dividend_received: self.dividend_received,
            last_transaction_date: self.last_transaction_date.clone(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct PendingMatch {
    pub sell_transaction_id: i64,
    pub buy_transaction_id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub buy_date: String,
    pub sell_date: String,
    pub buy_cost_per_share: f64,
    pub sell_price: f64,
    pub cost_basis: f64,
    pub sell_value: f64,
    pub realized_gain: f64,
}
pub fn replay_transactions(txns: &[Transaction]) -> Result<FifoReplay> {
    let mut states: HashMap<String, SymbolState> = HashMap::new();
    let mut symbol_order: Vec<String> = Vec::new();
    let mut pending_matches: Vec<PendingMatch> = Vec::new();

    for txn in txns {
        match txn.txn_type {
            TransactionType::OpeningBalance
            | TransactionType::Buy
            | TransactionType::MergerInvestment
            | TransactionType::DemergerInvestment
            | TransactionType::Rights => {
                let symbol = require_symbol(txn)?;
                note_symbol(&mut symbol_order, &symbol);
                let qty = require_positive_qty(txn.quantity, "quantity")?;
                let total_cost = buy_total_cost(txn)?;
                let state = states.entry(symbol).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());
                state.lots.push(Lot {
                    source_txn_id: txn.id,
                    acquired_date: txn.trade_date.clone(),
                    opened_qty: qty,
                    remaining_qty: qty,
                    total_cost,
                });
            }
            TransactionType::Sell
            | TransactionType::MergerRedemption
            | TransactionType::DemergerRedemption => {
                let symbol = require_symbol(txn)?;
                note_symbol(&mut symbol_order, &symbol);
                let sell_qty = require_positive_qty(txn.quantity, "sell quantity")?;
                let state = states.entry(symbol.clone()).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());

                let available = state.total_qty();
                if sell_qty > available + 1e-9 {
                    return Err(LedgerError(format!(
                        "sell quantity {sell_qty} exceeds available {available} for {symbol}"
                    )));
                }

                let sell_price = resolve_sell_price(txn, sell_qty, &state);

                let mut remaining = sell_qty;
                for lot in &mut state.lots {
                    if remaining <= 1e-9 {
                        break;
                    }
                    if lot.remaining_qty <= 1e-9 {
                        continue;
                    }
                    let matched = remaining.min(lot.remaining_qty);
                    let cost_per = lot.cost_per_share();
                    let cost_basis = matched * cost_per;
                    let sell_value = matched * sell_price;
                    let gain = sell_value - cost_basis;

                    pending_matches.push(PendingMatch {
                        sell_transaction_id: txn.id,
                        buy_transaction_id: lot.source_txn_id,
                        symbol: symbol.clone(),
                        quantity: matched,
                        buy_date: lot.acquired_date.clone(),
                        sell_date: txn.trade_date.clone(),
                        buy_cost_per_share: cost_per,
                        sell_price,
                        cost_basis,
                        sell_value,
                        realized_gain: gain,
                    });

                    lot.remaining_qty -= matched;
                    lot.total_cost -= cost_basis;
                    remaining -= matched;
                    state.realized_gain += gain;
                }
            }
            TransactionType::Split => {
                let symbol = require_symbol(txn)?;
                note_symbol(&mut symbol_order, &symbol);
                let (num, den) = split_ratio(txn)?;
                if num <= 0.0 || den <= 0.0 {
                    return Err(LedgerError("invalid split ratio".into()));
                }
                let factor = num / den;
                if let Some(key) = &txn.corporate_action_key {
                    let state = states.entry(symbol.clone()).or_default();
                    if state.applied_corp_actions.contains_key(key) {
                        continue;
                    }
                    state.applied_corp_actions.insert(key.clone(), ());
                }
                let state = states.entry(symbol).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());
                for lot in &mut state.lots {
                    lot.opened_qty *= factor;
                    lot.remaining_qty *= factor;
                    // total_cost unchanged; cost_per_share decreases
                }
            }
            TransactionType::Bonus => {
                let symbol = require_symbol(txn)?;
                note_symbol(&mut symbol_order, &symbol);
                if let Some(key) = &txn.corporate_action_key {
                    let state = states.entry(symbol.clone()).or_default();
                    if state.applied_corp_actions.contains_key(key) {
                        continue;
                    }
                    state.applied_corp_actions.insert(key.clone(), ());
                }
                let state = states.entry(symbol.clone()).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());

                if let (Some(bonus_num), Some(bonus_den)) =
                    (txn.bonus_ratio_num, txn.bonus_ratio_den)
                {
                    if bonus_num <= 0.0 || bonus_den <= 0.0 {
                        return Err(LedgerError("invalid bonus ratio".into()));
                    }
                    let factor = 1.0 + bonus_num / bonus_den;
                    for lot in &mut state.lots {
                        lot.opened_qty *= factor;
                        lot.remaining_qty *= factor;
                        // total_cost unchanged
                    }
                } else if let Some(qty) = txn.quantity.filter(|q| *q > 0.0) {
                    // Broker exports list bonus shares received without a ratio.
                    let lot = Lot {
                        source_txn_id: txn.id,
                        acquired_date: txn.trade_date.clone(),
                        opened_qty: qty,
                        remaining_qty: qty,
                        total_cost: 0.0,
                    };
                    state.lots.push(lot);
                } else {
                    return Err(LedgerError("bonus ratio num required".into()));
                }
            }
            TransactionType::Dividend => {
                let symbol = require_symbol(txn)?;
                note_symbol(&mut symbol_order, &symbol);
                let gross = txn.gross_amount.unwrap_or_else(|| {
                    let dps = txn.dividend_per_share.unwrap_or(0.0);
                    let qty = txn.eligible_quantity.unwrap_or_else(|| {
                        states.get(&symbol).map(|s| s.total_qty()).unwrap_or(0.0)
                    });
                    dps * qty
                });
                let state = states.entry(symbol).or_default();
                state.dividend_received += gross;
                state.last_transaction_date = Some(txn.trade_date.clone());
            }
            TransactionType::Sip | TransactionType::Swp => {}
        }
    }
    let symbols = states
        .iter()
        .map(|(sym, state)| (sym.clone(), state.to_stats()))
        .collect();
    Ok(FifoReplay {
        open_lots: open_lots_in_order(&states, &symbol_order),
        matches: pending_matches,
        symbols,
    })
}

fn note_symbol(order: &mut Vec<String>, symbol: &str) {
    if !order.iter().any(|s| s == symbol) {
        order.push(symbol.to_string());
    }
}

/// Remaining lots after sells, splits, and bonuses. Fully consumed lots are omitted.
/// Symbols follow first-seen order; lots within a symbol follow acquisition order.
fn open_lots_in_order(
    states: &HashMap<String, SymbolState>,
    symbol_order: &[String],
) -> Vec<(String, Lot)> {
    let mut out = Vec::new();
    for symbol in symbol_order {
        let Some(state) = states.get(symbol) else {
            continue;
        };
        for lot in &state.lots {
            if lot.remaining_qty > 1e-9 {
                out.push((symbol.clone(), lot.clone()));
            }
        }
    }
    out
}

fn require_symbol(txn: &Transaction) -> Result<String> {
    txn.symbol
        .clone()
        .ok_or_else(|| LedgerError(format!("transaction {} requires symbol", txn.id)))
}

fn require_positive_qty(qty: Option<f64>, field: &str) -> Result<f64> {
    let q = qty.ok_or_else(|| LedgerError(format!("{field} is required")))?;
    if q <= 0.0 {
        return Err(LedgerError(format!("{field} must be positive")));
    }
    Ok(q)
}

fn buy_total_cost(txn: &Transaction) -> Result<f64> {
    if let Some(net) = txn.net_amount {
        return Ok(net);
    }
    let qty = require_positive_qty(txn.quantity, "quantity")?;
    let price = txn.price.unwrap_or(0.0);
    let gross = txn.gross_amount.unwrap_or(qty * price);
    let brokerage = txn.brokerage.unwrap_or(0.0);
    let taxes = txn.taxes.unwrap_or(0.0);
    Ok(gross + brokerage + taxes)
}

fn resolve_sell_price(txn: &Transaction, sell_qty: f64, state: &SymbolState) -> f64 {
    if let Some(price) = txn.price.map(f64::abs).filter(|p| *p > 0.0) {
        return price;
    }
    if let Some(gross) = txn.gross_amount.map(f64::abs).filter(|g| *g > 0.0) {
        return gross / sell_qty;
    }
    if let Some(net) = txn.net_amount.map(f64::abs).filter(|n| *n > 0.0) {
        return net / sell_qty;
    }
    if matches!(
        txn.txn_type,
        TransactionType::DemergerRedemption | TransactionType::MergerRedemption
    ) {
        let total_cost = state.total_cost();
        let total_qty = state.total_qty();
        if total_qty > 1e-9 {
            return total_cost / total_qty;
        }
    }
    0.0
}

fn split_ratio(txn: &Transaction) -> Result<(f64, f64)> {
    if let Some(num) = txn.split_ratio_num.filter(|n| *n > 0.0) {
        let den = txn.split_ratio_den.filter(|d| *d > 0.0).unwrap_or(1.0);
        return Ok((num, den));
    }
    if let Some(qty) = txn.quantity.filter(|q| *q > 0.0) {
        return Ok((qty, 1.0));
    }
    Err(LedgerError("split ratio num required".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::models::Transaction;

    fn txn(
        id: i64,
        kind: TransactionType,
        qty: Option<f64>,
        price: Option<f64>,
        net: Option<f64>,
        split_num: Option<f64>,
    ) -> Transaction {
        Transaction {
            id,
            user_id: 1,
            portfolio_id: 1,
            txn_type: kind,
            trade_date: format!("2024-01-{id:02}"),
            symbol: Some("ITC.NS".into()),
            quantity: qty,
            price,
            gross_amount: None,
            brokerage: None,
            taxes: None,
            net_amount: net,
            split_ratio_num: split_num,
            split_ratio_den: split_num.map(|_| 1.0),
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
            source: "test".into(),
            corporate_action_key: None,
            schedule_id: None,
            created_at: 0,
            updated_at: 0,
            labels: vec![],
        }
    }

    #[test]
    fn split_updates_open_lot_quantity() {
        let replay = replay_transactions(&[
            txn(1, TransactionType::Buy, Some(10.0), Some(1000.0), Some(10000.0), None),
            txn(2, TransactionType::Split, None, None, None, Some(5.0)),
        ])
        .unwrap();
        assert_eq!(replay.open_lots.len(), 1);
        let (_, lot) = &replay.open_lots[0];
        assert!((lot.opened_qty - 50.0).abs() < 1e-9);
        assert!((lot.remaining_qty - 50.0).abs() < 1e-9);
        assert!((lot.total_cost - 10000.0).abs() < 1e-9);
        assert!((lot.cost_per_share() - 200.0).abs() < 1e-9);
        assert!((replay.symbols["ITC.NS"].quantity - 50.0).abs() < 1e-9);
    }

    #[test]
    fn sell_drops_closed_lots_and_reduces_the_remainder() {
        let replay = replay_transactions(&[
            txn(1, TransactionType::Buy, Some(10.0), Some(100.0), Some(1000.0), None),
            txn(2, TransactionType::Buy, Some(10.0), Some(150.0), Some(1500.0), None),
            txn(3, TransactionType::Sell, Some(12.0), Some(200.0), None, None),
        ])
        .unwrap();
        assert_eq!(replay.open_lots.len(), 1);
        let (_, lot) = &replay.open_lots[0];
        assert_eq!(lot.source_txn_id, 2);
        assert!((lot.opened_qty - 10.0).abs() < 1e-9);
        assert!((lot.remaining_qty - 8.0).abs() < 1e-9);
        assert!((lot.total_cost - 1200.0).abs() < 1e-6);
        assert!((replay.symbols["ITC.NS"].quantity - 8.0).abs() < 1e-9);
    }
}
