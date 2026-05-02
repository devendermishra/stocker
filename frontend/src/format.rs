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
