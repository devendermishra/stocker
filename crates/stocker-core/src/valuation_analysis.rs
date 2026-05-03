use crate::models::{
    ChartHistory, EarningsBasedValue, Financials, FundamentalAnalysis, HistoricalMultiples,
    IncomeStatementRow, PeerQuote, PeerValuationCompare, StatementBundle, ValuationAnalysis,
};

fn median(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = xs.len();
    Some(if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    })
}

fn closest_close(chart: &ChartHistory, target_ts: i64) -> Option<f64> {
    if chart.bars.is_empty() {
        return None;
    }
    let mut best: Option<(i64, f64)> = None;
    for b in &chart.bars {
        let d = (b.ts - target_ts).abs();
        match best {
            None => best = Some((d, b.close)),
            Some((bd, _)) if d < bd => best = Some((d, b.close)),
            _ => {}
        }
    }
    best.map(|(_, c)| c)
}

fn historical_pe_series(chart: &ChartHistory, income: &[IncomeStatementRow], _shares: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for row in income {
        let eps = match row.diluted_eps {
            Some(e) if e > 0.0 => e,
            _ => continue,
        };
        let ts = match row.end_ts {
            Some(t) => t,
            None => continue,
        };
        let Some(cl) = closest_close(chart, ts) else { continue };
        if cl > 0.0 {
            out.push(cl / eps);
        }
    }
    out
}

fn historical_pb_series(
    chart: &ChartHistory,
    bundle: &StatementBundle,
    shares: f64,
) -> Vec<f64> {
    if shares <= 0.0 {
        return Vec::new();
    }
    let mut bal = bundle.balance_annual.clone();
    bal.sort_by(|a, b| {
        a.end_ts
            .unwrap_or(0)
            .cmp(&b.end_ts.unwrap_or(0))
    });
    let mut out = Vec::new();
    for b in bal {
        let bvps = b.total_equity / shares;
        if bvps <= 0.0 {
            continue;
        }
        let ts = match b.end_ts {
            Some(t) => t,
            None => continue,
        };
        let Some(cl) = closest_close(chart, ts) else { continue };
        if cl > 0.0 {
            out.push(cl / bvps);
        }
    }
    out
}

fn classify_vs_median(current: f64, med: Option<f64>) -> Option<&'static str> {
    let m = med?;
    if m <= 0.0 || current <= 0.0 {
        return None;
    }
    let r = current / m;
    Some(if r >= 1.5 {
        "Very expensive vs historical median (≥50% above median)"
    } else if r > 1.2 {
        "Expensive vs historical median (>20% above median)"
    } else if r >= 0.8 {
        "Fair vs historical median (within ±20% of median)"
    } else {
        "Cheap vs historical median (≥20% below median)"
    })
}

fn peer_median<F: Fn(&PeerQuote) -> Option<f64>>(peers: &[PeerQuote], f: F) -> Option<f64> {
    let xs: Vec<f64> = peers.iter().filter_map(|p| f(p)).filter(|x| x.is_finite() && *x > 0.0).collect();
    median(xs)
}

pub fn build_valuation_analysis(
    price: f64,
    financials: &Financials,
    chart: &ChartHistory,
    bundle: &StatementBundle,
    subject: &PeerQuote,
    peers: &[PeerQuote],
    fundamental: &FundamentalAnalysis,
) -> ValuationAnalysis {
    let mut inc = bundle.income_annual.clone();
    inc.sort_by(|a, b| a.end_ts.unwrap_or(0).cmp(&b.end_ts.unwrap_or(0)));

    let pe_hist = historical_pe_series(chart, &inc, financials.shares_outstanding);
    let n = pe_hist.len();
    let med_pe_3 = if n >= 3 {
        median(pe_hist[n - 3..].to_vec())
    } else {
        None
    };
    let med_pe_5 = if n >= 5 {
        median(pe_hist[n - 5..].to_vec())
    } else {
        None
    };

    let pb_hist = historical_pb_series(chart, bundle, financials.shares_outstanding);
    let npb = pb_hist.len();
    let med_pb_3 = if npb >= 3 {
        median(pb_hist[npb - 3..].to_vec())
    } else {
        None
    };
    let med_pb_5 = if npb >= 5 {
        median(pb_hist[npb - 5..].to_vec())
    } else {
        None
    };

    let historical = HistoricalMultiples {
        median_pe_3y: med_pe_3,
        median_pe_5y: med_pe_5,
        median_pb_3y: med_pb_3,
        median_pb_5y: med_pb_5,
        median_ev_ebitda_3y: None,
        median_ev_ebitda_5y: None,
        pe_points_used: n,
        pb_points_used: npb,
        ev_ebitda_points_used: 0,
    };

    let cur_pe = financials.pe_ratio;
    let cur_pb = financials.price_to_book;

    let hist_class = if cur_pe > 0.0 {
        classify_vs_median(cur_pe, med_pe_5.or(med_pe_3))
            .unwrap_or("Insufficient historical P/E series")
    } else if cur_pb > 0.0 {
        classify_vs_median(cur_pb, med_pb_5.or(med_pb_3))
            .unwrap_or("Insufficient historical P/B series")
    } else {
        "Insufficient data for historical multiple classification"
    }
    .to_string();

    let m_pe = peer_median(peers, |p| (p.pe_ratio > 0.0).then_some(p.pe_ratio));
    let m_pb = peer_median(peers, |p| (p.price_to_book > 0.0).then_some(p.price_to_book));
    let m_ev = peer_median(peers, |p| p.ev_to_ebitda);
    let m_ps = peer_median(peers, |p| (p.price_to_sales > 0.0).then_some(p.price_to_sales));
    let m_roe = peer_median(peers, |p| Some(p.return_on_equity));
    let m_roce = peer_median(peers, |p| p.return_on_capital_employed);
    let m_rg = peer_median(peers, |p| Some(p.revenue_growth));
    let m_pg = peer_median(peers, |p| Some(p.pat_growth));

    let pct_vs = |cur: f64, med: Option<f64>| -> Option<f64> {
        let m = med?;
        if m.abs() < 1e-9 {
            return None;
        }
        Some(((cur / m) - 1.0) * 100.0)
    };

    let peer_interp = {
        let pr = subject.pe_ratio;
        let rg = subject.revenue_growth;
        let roe = subject.return_on_equity;
        let mpe = m_pe.unwrap_or(0.0);
        let mrg = m_rg.unwrap_or(0.0);
        let mroe = m_roe.unwrap_or(0.0);
        if mpe > 0.0 && pr > mpe * 1.15 && rg >= mrg && roe >= mroe {
            "Premium may be justified: higher multiple with growth/ROE at or above peer median."
        } else if mpe > 0.0 && pr > mpe * 1.15 {
            "Overvalued vs peers on P/E without clearly better growth/ROE."
        } else if mpe > 0.0 && pr < mpe * 0.85 && rg >= mrg * 0.9 && roe >= mroe * 0.9 {
            "Undervalued vs peers with similar or better growth/ROE (heuristic)."
        } else if mpe > 0.0 && pr < mpe * 0.85 {
            "Cheap vs peers — check for value-trap fundamentals (growth, ROCE, leverage, FCF)."
        } else {
            "Peer multiples near the median band for this sample."
        }
        .to_string()
    };

    let value_trap = fundamental.growth.interpretation.contains("Weak")
        || fundamental.balance_sheet.interpretation.contains("Risky")
        || fundamental.cash_flow.interpretation.contains("Weak");

    let peer_value_read = if peer_interp.contains("Cheap") && value_trap {
        "Possible value trap: cheap vs peers but weak growth, balance sheet, or cash conversion."
    } else {
        peer_interp.as_str()
    }
    .to_string();

    let peg = {
        let pe = financials.pe_ratio;
        let g = financials.earnings_growth;
        if pe > 0.0 && g > 0.0 {
            Some(pe / (g * 100.0))
        } else {
            None
        }
    };

    let ev = financials.enterprise_value;
    let ev_ebitda = if ev > 0.0 && financials.ebitda > 0.0 {
        Some(ev / financials.ebitda)
    } else {
        subject.ev_to_ebitda
    };
    let ev_sales = if ev > 0.0 && financials.revenue > 0.0 {
        Some(ev / financials.revenue)
    } else {
        subject.ev_to_sales
    };

    let mcap_sales = if financials.market_cap > 0.0 && financials.revenue > 0.0 {
        Some(financials.market_cap / financials.revenue)
    } else {
        None
    };

    let earn_y = if price > 0.0 && financials.trailing_eps > 0.0 {
        Some((financials.trailing_eps / price) * 100.0)
    } else if cur_pe > 0.0 {
        Some(100.0 / cur_pe)
    } else {
        None
    };

    let fcf_y = if financials.market_cap > 0.0 {
        Some((financials.free_cashflow / financials.market_cap) * 100.0)
    } else {
        None
    };

    let fair_pe = med_pe_5.or(med_pe_3).unwrap_or(18.0).clamp(8.0, 45.0);
    let g = financials.earnings_growth.max(0.03).min(0.35);
    let eps = financials.trailing_eps;
    let mos = 0.15_f64;
    let base_fv = if eps > 0.0 {
        eps * fair_pe
    } else {
        0.0
    };
    let fair_value = base_fv * (1.0 - mos);
    let upside = if price > 0.0 && fair_value > 0.0 {
        Some(((fair_value / price) - 1.0) * 100.0)
    } else {
        None
    };
    let bull = if eps > 0.0 {
        eps * fair_pe * 1.2
    } else {
        0.0
    };
    let bear = if eps > 0.0 {
        eps * fair_pe * 0.75 * (1.0 - g)
    } else {
        0.0
    };

    let earnings_based = EarningsBasedValue {
        input_eps: eps,
        input_growth_rate: g,
        fair_pe,
        margin_of_safety: mos,
        fair_value,
        upside_downside_pct: upside,
        bull_value: bull,
        base_value: base_fv,
        bear_value: bear,
    };

    let valuation_label = {
        let cheap_hist = hist_class.contains("Cheap") || hist_class.contains("Fair") && cur_pe > 0.0 && med_pe_5.map(|m| cur_pe < m * 0.85).unwrap_or(false);
        let exp_hist = hist_class.contains("Very expensive") || hist_class.contains("Expensive");
        let exp_peer = m_pe.map(|m| cur_pe > m * 1.2).unwrap_or(false);
        let cheap_peer = m_pe.map(|m| cur_pe > 0.0 && cur_pe < m * 0.8).unwrap_or(false);
        let qual_ok = !fundamental.profitability.interpretation.contains("Poor")
            && !fundamental.balance_sheet.interpretation.contains("Risky");

        if value_trap && cheap_peer {
            "Avoid / Possible Value Trap".to_string()
        } else if cheap_hist && cheap_peer && qual_ok {
            "Very Cheap".to_string()
        } else if cheap_hist || cheap_peer {
            "Cheap".to_string()
        } else if exp_hist && exp_peer {
            "Very Expensive".to_string()
        } else if exp_hist || exp_peer {
            "Expensive".to_string()
        } else {
            "Fairly Valued".to_string()
        }
    };

    let conf = if n >= 3 && !peers.is_empty() {
        "High"
    } else if n >= 1 || !peers.is_empty() {
        "Medium"
    } else {
        "Low"
    }
    .to_string();

    ValuationAnalysis {
        pe: cur_pe,
        forward_pe: financials.forward_pe,
        price_to_book: cur_pb,
        price_to_sales: financials.price_to_sales,
        ev_to_ebitda: ev_ebitda,
        ev_to_sales: ev_sales,
        peg_ratio: peg,
        dividend_yield: financials.dividend_yield,
        earnings_yield_pct: earn_y,
        fcf_yield_pct: fcf_y,
        market_cap_to_sales: mcap_sales,
        historical,
        historical_classification: hist_class,
        peer_compare: PeerValuationCompare {
            median_pe: m_pe,
            median_pb: m_pb,
            median_ev_ebitda: m_ev,
            median_ps: m_ps,
            median_roe: m_roe,
            median_roce: m_roce,
            median_revenue_growth: m_rg,
            median_profit_growth: m_pg,
            subject_pe_vs_median_pct: pct_vs(subject.pe_ratio, m_pe),
            subject_pb_vs_median_pct: pct_vs(subject.price_to_book, m_pb),
            subject_ev_ebitda_vs_median_pct: match (subject.ev_to_ebitda, m_ev) {
                (Some(a), Some(b)) => pct_vs(a, Some(b)),
                _ => None,
            },
            subject_ps_vs_median_pct: pct_vs(subject.price_to_sales, m_ps),
            interpretation: peer_interp,
        },
        earnings_based,
        valuation_label,
        peer_value_read,
        confidence: conf,
    }
}
