pub fn fmt_price_in_currency(price: f64, currency: Option<&str>) -> String {
    let sym = match currency.unwrap_or("INR") {
        "USD" => "$",
        "GBP" => "£",
        "EUR" => "€",
        _ => "₹",
    };
    format!("{sym}{p:.2}", sym = sym, p = price)
}

pub fn fmt_money(v: f64) -> String {
    if v == 0.0 {
        return "N/A".to_string();
    }
    let a = v.abs();
    if a >= 1e12 {
        format!("₹{:.2}T", v / 1e12)
    } else if a >= 1e9 {
        format!("₹{:.2}B", v / 1e9)
    } else if a >= 1e7 {
        format!("₹{:.2}Cr", v / 1e7)
    } else if a >= 1e5 {
        format!("₹{:.2}L", v / 1e5)
    } else {
        format!("₹{:.2}", v)
    }
}

pub fn fmt_opt_money(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => fmt_money(x),
        _ => "N/A".to_string(),
    }
}

pub fn fmt_pct(v: f64) -> String {
    format!("{:.2}%", v * 100.0)
}

pub fn fmt_opt_pct(v: Option<f64>) -> String {
    v.map(|x| format!("{:.2}%", x))
        .unwrap_or_else(|| "N/A".to_string())
}

pub fn fmt_opt_num(v: Option<f64>) -> String {
    v.map(|x| format!("{:.2}", x))
        .unwrap_or_else(|| "N/A".to_string())
}

pub fn fmt_opt_ratio(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => fmt_pct(x),
        _ => "N/A".to_string(),
    }
}

pub fn fmt_opt_multiple(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() && x > 0.0 => format!("{:.2}", x),
        _ => "N/A".to_string(),
    }
}

/// Format a screener snapshot value using catalog unit hints.
pub fn fmt_screener_metric(value: Option<f64>, unit: &str) -> String {
    let Some(v) = value else {
        return "—".to_string();
    };
    if !v.is_finite() {
        return "—".to_string();
    }
    match unit {
        "Rupees" => fmt_price_in_currency(v, Some("INR")),
        "RupeesCr" => fmt_money(v),
        "Percent" => format!("{:.2}%", v),
        "Ratio" => format!("{:.2}%", v * 100.0),
        "Multiple" => format!("{:.2}", v),
        "Count" => {
            if v.abs() >= 1e7 {
                format!("{:.2}Cr", v / 1e7)
            } else if v.abs() >= 1e3 {
                format!("{:.2}K", v / 1e3)
            } else {
                format!("{:.0}", v)
            }
        }
        "Score" => format!("{:.2}", v),
        "Days" => format!("{:.1} days", v),
        _ => format!("{:.2}", v),
    }
}

pub fn fmt_refreshed_at(ts: Option<i64>) -> String {
    use chrono::TimeZone;

    let Some(ts) = ts else {
        return "Never".to_string();
    };
    chrono::Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| ts.to_string())
}
