//! Technical indicators from daily OHLCV (Yahoo chart). Heuristic only — not trading advice.

use crate::models::{
    ChartHistory, Financials, TechnicalAnalysis, TechnicalMomentum, TechnicalTrend,
    TechnicalVolatility, TechnicalVolume,
};

pub fn sma_last(closes: &[f64], period: usize) -> Option<f64> {
    if period == 0 || closes.len() < period {
        return None;
    }
    let slice = &closes[closes.len() - period..];
    let s: f64 = slice.iter().sum();
    Some(s / period as f64)
}

pub fn rsi14(closes: &[f64]) -> Option<f64> {
    if closes.len() < 15 {
        return None;
    }
    let n = closes.len();
    let mut gains = 0.0;
    let mut losses = 0.0;
    for i in (n - 14)..n {
        let d = closes[i] - closes[i - 1];
        if d >= 0.0 {
            gains += d;
        } else {
            losses -= d;
        }
    }
    let avg_g = gains / 14.0;
    let avg_l = losses / 14.0;
    if avg_l < 1e-12 {
        return Some(100.0);
    }
    let rs = avg_g / avg_l;
    Some(100.0 - (100.0 / (1.0 + rs)))
}

fn ema_walk(closes: &[f64], span: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![f64::NAN; n];
    if n < span {
        return out;
    }
    let alpha = 2.0 / (span as f64 + 1.0);
    let mut e = closes[0..span].iter().sum::<f64>() / span as f64;
    out[span - 1] = e;
    for i in span..n {
        e = alpha * closes[i] + (1.0 - alpha) * e;
        out[i] = e;
    }
    out
}

pub fn macd_components(closes: &[f64]) -> (Option<f64>, Option<f64>, Option<f64>) {
    if closes.len() < 35 {
        return (None, None, None);
    }
    let e12 = ema_walk(closes, 12);
    let e26 = ema_walk(closes, 26);
    let n = closes.len();
    let mut macd_line = Vec::with_capacity(n);
    for i in 0..n {
        let a = e12[i];
        let b = e26[i];
        if a.is_finite() && b.is_finite() {
            macd_line.push(a - b);
        } else {
            macd_line.push(f64::NAN);
        }
    }
    let first_fin = macd_line.iter().position(|x| x.is_finite());
    let Some(start) = first_fin else {
        return (None, None, None);
    };
    let m_slice: Vec<f64> = macd_line[start..].iter().copied().filter(|x| x.is_finite()).collect();
    if m_slice.len() < 10 {
        let ml = match macd_line.last().copied().filter(|x| x.is_finite()) {
            Some(m) => m,
            None => return (None, None, None),
        };
        return (Some(ml), None, None);
    }
    let sig_line = ema_walk(&m_slice, 9);
    let ml = match m_slice.last().copied() {
        Some(m) => m,
        None => return (None, None, None),
    };
    let sig = match sig_line.last().copied().filter(|x| x.is_finite()) {
        Some(s) => s,
        None => return (Some(ml), None, None),
    };
    let hist = ml - sig;
    (Some(ml), Some(sig), Some(hist))
}

pub fn roc_pct(closes: &[f64], trading_days: usize) -> Option<f64> {
    if trading_days == 0 || closes.len() <= trading_days {
        return None;
    }
    let old = closes[closes.len() - 1 - trading_days];
    let new = *closes.last()?;
    if old <= 0.0 {
        return None;
    }
    Some(((new / old) - 1.0) * 100.0)
}

pub fn annualized_volatility(closes: &[f64], window: usize) -> Option<f64> {
    if window < 5 || closes.len() <= window {
        return None;
    }
    let slice = &closes[closes.len() - window..];
    let mut rets = Vec::new();
    for i in 1..slice.len() {
        let a = slice[i - 1];
        let b = slice[i];
        if a > 0.0 {
            rets.push((b / a).ln());
        }
    }
    if rets.len() < 5 {
        return None;
    }
    let mean: f64 = rets.iter().sum::<f64>() / rets.len() as f64;
    let var: f64 = rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / rets.len() as f64;
    let daily = var.sqrt();
    Some(daily * (252.0_f64).sqrt() * 100.0)
}

pub fn max_drawdown_pct(closes: &[f64]) -> Option<f64> {
    if closes.len() < 2 {
        return None;
    }
    let mut peak = closes[0];
    let mut max_dd = 0.0_f64;
    for &c in closes.iter().skip(1) {
        if c > peak {
            peak = c;
        }
        if peak > 0.0 {
            let dd = (c / peak) - 1.0;
            if dd < max_dd {
                max_dd = dd;
            }
        }
    }
    Some(max_dd * 100.0)
}

pub fn atr14(bars: &[crate::models::ChartBar]) -> Option<f64> {
    if bars.len() < 15 {
        return None;
    }
    let slice = &bars[bars.len() - 15..];
    let mut trs = Vec::new();
    for i in 1..slice.len() {
        let h = slice[i].high;
        let l = slice[i].low;
        let pc = slice[i - 1].close;
        let tr = (h - l).max((h - pc).abs()).max((l - pc).abs());
        trs.push(tr);
    }
    if trs.is_empty() {
        return None;
    }
    Some(trs.iter().sum::<f64>() / trs.len() as f64)
}

fn closes_from_chart(chart: &ChartHistory) -> Vec<f64> {
    chart.bars.iter().map(|b| b.close).filter(|c| *c > 0.0).collect()
}

fn rsi_label(rsi: Option<f64>) -> String {
    let Some(r) = rsi else {
        return "Insufficient data".to_string();
    };
    if r < 30.0 {
        "Oversold".to_string()
    } else if r < 45.0 {
        "Weak".to_string()
    } else if r < 60.0 {
        "Neutral".to_string()
    } else if r < 70.0 {
        "Strong".to_string()
    } else {
        "Overbought".to_string()
    }
}

fn trend_label(price: f64, sma50: Option<f64>, sma200: Option<f64>) -> String {
    match (sma50, sma200) {
        (Some(s50), Some(s200)) if s50 > s200 && price > s50 && price > s200 => {
            "Strong uptrend (price above 50/200 DMA, 50 above 200)".to_string()
        }
        (Some(s50), Some(s200)) if price < s50 && price < s200 => {
            "Weak trend (price below 50 and 200 DMA)".to_string()
        }
        (Some(s50), Some(s200)) if price > s50 && price < s200 => {
            "Recovery (above 50 DMA, below 200 DMA)".to_string()
        }
        (_, Some(s200)) if price > s200 => "Long-term bullish (above 200 DMA)".to_string(),
        (_, Some(s200)) if price < s200 => "Long-term bearish (below 200 DMA)".to_string(),
        _ => "Trend unclear (missing moving averages)".to_string(),
    }
}

pub fn build_technical_analysis(price: f64, financials: &Financials, chart: &ChartHistory) -> TechnicalAnalysis {
    let closes = closes_from_chart(chart);
    let confidence = if closes.len() < 60 {
        "Low"
    } else if closes.len() < 200 {
        "Medium"
    } else {
        "High"
    }
    .to_string();

    let sma20 = sma_last(&closes, 20);
    let sma50 = sma_last(&closes, 50);
    let sma100 = sma_last(&closes, 100);
    let sma200 = sma_last(&closes, 200);

    let pv50 = match (price > 0.0, sma50) {
        (true, Some(s)) => Some(((price / s) - 1.0) * 100.0),
        _ => None,
    };
    let pv200 = match (price > 0.0, sma200) {
        (true, Some(s)) => Some(((price / s) - 1.0) * 100.0),
        _ => None,
    };

    let rsi = rsi14(&closes);
    let (macd, macd_sig, macd_hist) = macd_components(&closes);

    let trend = TechnicalTrend {
        sma_20: sma20,
        sma_50: sma50,
        sma_100: sma100,
        sma_200: sma200,
        price_vs_sma50_pct: pv50,
        price_vs_sma200_pct: pv200,
        trend_label: trend_label(price, sma50, sma200),
    };

    let momentum = TechnicalMomentum {
        rsi_14: rsi,
        macd,
        macd_signal: macd_sig,
        macd_histogram: macd_hist,
        rsi_label: rsi_label(rsi),
        roc_1m_pct: roc_pct(&closes, 21),
        roc_3m_pct: roc_pct(&closes, 63),
        roc_6m_pct: roc_pct(&closes, 126),
        roc_1y_pct: roc_pct(&closes, 252),
    };

    let hi = financials.fifty_two_week_high;
    let lo = financials.fifty_two_week_low;
    let dist_hi = if hi > 0.0 && price > 0.0 {
        Some(((hi - price) / hi) * 100.0)
    } else {
        None
    };
    let dist_lo = if lo > 0.0 && price > 0.0 {
        Some(((price - lo) / lo) * 100.0)
    } else {
        None
    };

    let vol_note = {
        let mut n = String::new();
        if let Some(d) = dist_hi {
            if d < 5.0 {
                n.push_str("Near 52-week high — momentum strong but possibly extended. ");
            }
        }
        if let Some(d) = dist_lo {
            if d < 10.0 && dist_hi.unwrap_or(100.0) > 15.0 {
                n.push_str("Closer to 52-week low — check whether weakness is price-only or fundamental. ");
            }
        }
        if n.is_empty() {
            "Volatility context from Yahoo 52-week range.".to_string()
        } else {
            n
        }
    };

    let vol_1y = annualized_volatility(&closes, closes.len().min(252));
    let mdd = max_drawdown_pct(&closes);
    let atr = atr14(&chart.bars);

    let volatility = TechnicalVolatility {
        fifty_two_week_high: hi,
        fifty_two_week_low: lo,
        dist_from_high_pct: dist_hi,
        dist_from_low_pct: dist_lo,
        vol_1y_ann_pct: vol_1y,
        max_drawdown_1y_pct: mdd,
        atr_14: atr,
        note: vol_note,
    };

    let avg_vol = if financials.average_volume_10_day > 0.0 {
        financials.average_volume_10_day
    } else {
        let n = closes.len().min(20).max(1);
        chart.bars.iter().rev().take(n).map(|b| b.volume).sum::<f64>() / n as f64
    };
    let cur_vol = if financials.regular_market_volume > 0.0 {
        financials.regular_market_volume
    } else {
        chart.bars.last().map(|b| b.volume).unwrap_or(0.0)
    };
    let vs20 = if chart.bars.len() >= 21 {
        let sum20: f64 = chart.bars.iter().rev().take(20).map(|b| b.volume).sum();
        let m = sum20 / 20.0;
        if m > 0.0 {
            Some(((cur_vol / m) - 1.0) * 100.0)
        } else {
            None
        }
    } else {
        None
    };

    let breakout = {
        let roc1m = roc_pct(&closes, 21).unwrap_or(0.0);
        let hi_v = vs20.unwrap_or(0.0) > 25.0;
        roc1m > 5.0 && hi_v
    };

    let vol_interp = if breakout {
        "Recent price strength with above-average volume (heuristic breakout flag)."
    } else if vs20.unwrap_or(0.0) < -20.0 {
        "Volume below recent average — liquidity may be thin."
    } else {
        "Volume regime looks normal vs recent history."
    }
    .to_string();

    let volume = TechnicalVolume {
        average_volume: avg_vol,
        current_volume: cur_vol,
        vs_20d_avg_pct: vs20,
        delivery_pct: None,
        volume_breakout: breakout,
        interpretation: vol_interp,
    };

    TechnicalAnalysis {
        trend,
        momentum,
        volatility,
        volume,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_closes(n: usize, start: f64, step: f64) -> Vec<f64> {
        (0..n).map(|i| start + step * i as f64).collect()
    }

    #[test]
    fn sma_middle_value() {
        let c = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((sma_last(&c, 3).unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn rsi_bounded() {
        let mut c = linear_closes(30, 10.0, 0.5);
        let r = rsi14(&c).unwrap();
        assert!(r >= 0.0 && r <= 100.0);
        // strong down
        c = (0..30).map(|i| 30.0 - i as f64 * 0.4).collect();
        let r2 = rsi14(&c).unwrap();
        assert!(r2 < 50.0);
    }

    #[test]
    fn roc_positive_on_rally() {
        let c = linear_closes(40, 10.0, 0.2);
        let r = roc_pct(&c, 21).unwrap();
        assert!(r > 0.0);
    }
}
