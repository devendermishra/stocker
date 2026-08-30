//! Download a research report as JSON, XML, or a text PDF.

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy)]
pub enum ReportExportFormat {
    Json,
    Xml,
    Pdf,
}

impl ReportExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Pdf => "pdf",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Xml => "application/xml",
            Self::Pdf => "application/pdf",
        }
    }
}

pub fn export_filename(symbol: &str, format: ReportExportFormat) -> String {
    let stem: String = symbol
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = if stem.is_empty() {
        "report".to_string()
    } else {
        stem
    };
    format!("{stem}_research_report.{}", format.extension())
}

pub fn report_export_bytes<T: Serialize>(
    report: &T,
    format: ReportExportFormat,
) -> Result<Vec<u8>, String> {
    let value = serde_json::to_value(report).map_err(|e| e.to_string())?;
    match format {
        ReportExportFormat::Json => serde_json::to_vec_pretty(&value).map_err(|e| e.to_string()),
        ReportExportFormat::Xml => Ok(value_to_xml("research_report", &value).into_bytes()),
        ReportExportFormat::Pdf => Ok(report_pdf_bytes(&value)),
    }
}

pub fn save_export(filename: &str, mime: &str, bytes: &[u8]) -> Result<String, String> {
    #[cfg(feature = "web")]
    {
        download_in_browser(filename, mime, bytes)?;
        return Ok(format!("Downloaded {filename}"));
    }

    #[cfg(feature = "desktop")]
    {
        let _ = mime;
        let path = desktop_save_path(filename)?;
        std::fs::write(&path, bytes).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
        return Ok(format!("Saved {}", path.display()));
    }

    #[allow(unreachable_code)]
    {
        let _ = (filename, mime, bytes);
        Err("Export is unavailable in this build".to_string())
    }
}

#[cfg(feature = "desktop")]
fn desktop_save_path(filename: &str) -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let downloads = home.join("Downloads");
    let dir = if downloads.is_dir() { downloads } else { home };
    Ok(dir.join(filename))
}

#[cfg(feature = "web")]
fn download_in_browser(filename: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    use base64::Engine;
    use dioxus::prelude::spawn;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let js = format!(
        r#"(function(){{
            const b64 = "{b64}";
            const bin = Uint8Array.from(atob(b64), function(c) {{ return c.charCodeAt(0); }});
            const blob = new Blob([bin], {{ type: "{mime}" }});
            const a = document.createElement("a");
            a.href = URL.createObjectURL(blob);
            a.download = "{filename}";
            document.body.appendChild(a);
            a.click();
            a.remove();
            URL.revokeObjectURL(a.href);
        }})()"#
    );
    spawn(async move {
        let _ = dioxus::document::eval(&js).await;
    });
    Ok(())
}

pub fn value_to_xml(root: &str, value: &Value) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_xml(&mut out, root, value, 0);
    out
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_name(raw: &str) -> String {
    let mut n: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if n.is_empty() {
        n = "field".into();
    }
    if n.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        n = format!("n{n}");
    }
    n
}

fn write_xml(out: &mut String, name: &str, value: &Value, indent: usize) {
    let pad = "  ".repeat(indent);
    let tag = xml_name(name);
    match value {
        Value::Null => {
            out.push_str(&pad);
            out.push_str(&format!("<{tag}/>\n"));
        }
        Value::Bool(b) => {
            out.push_str(&pad);
            out.push_str(&format!("<{tag}>{}</{tag}>\n", b));
        }
        Value::Number(n) => {
            out.push_str(&pad);
            out.push_str(&format!("<{tag}>{n}</{tag}>\n"));
        }
        Value::String(s) => {
            out.push_str(&pad);
            out.push_str(&format!("<{tag}>{}</{tag}>\n", xml_escape(s)));
        }
        Value::Array(items) => {
            out.push_str(&pad);
            out.push_str(&format!("<{tag}>\n"));
            for item in items {
                write_xml(out, "item", item, indent + 1);
            }
            out.push_str(&pad);
            out.push_str(&format!("</{tag}>\n"));
        }
        Value::Object(map) => {
            out.push_str(&pad);
            out.push_str(&format!("<{tag}>\n"));
            for (k, v) in map {
                write_xml(out, k, v, indent + 1);
            }
            out.push_str(&pad);
            out.push_str(&format!("</{tag}>\n"));
        }
    }
}

fn json_text(v: &Value, keys: &[&str]) -> Option<String> {
    let mut cur = v;
    for k in keys {
        cur = cur.get(*k)?;
    }
    match cur {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn json_list(v: &Value, keys: &[&str]) -> Vec<String> {
    let mut cur = v;
    for k in keys {
        let Some(next) = cur.get(*k) else {
            return Vec::new();
        };
        cur = next;
    }
    match cur {
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn report_plain_text(v: &Value) -> String {
    let mut lines = Vec::new();
    let symbol = json_text(v, &["symbol"]).unwrap_or_else(|| "report".into());
    let name = json_text(v, &["long_name"]).unwrap_or_else(|| symbol.clone());
    lines.push("Stocker research report".to_string());
    lines.push(format!("{name} ({symbol})"));
    if let Some(p) = json_text(v, &["price"]) {
        lines.push(format!("Last price: {p}"));
    }
    if let Some(t) = json_text(v, &["retrieved_at"]) {
        lines.push(format!("Live snapshot as of: {t}"));
    }
    lines.push(String::new());
    if let Some(s) = json_text(v, &["report_insights", "executive_summary"]) {
        lines.push("Executive summary".to_string());
        lines.push(s);
        lines.push(String::new());
    }
    if let Some(s) = json_text(v, &["research_rating", "rating_label"]) {
        let score = json_text(v, &["research_rating", "overall_score"])
            .or_else(|| json_text(v, &["research_rating", "provisional_screening_score"]))
            .unwrap_or_else(|| "N/A".to_string());
        lines.push(format!("Rating: {s} (overall {score})"));
    }
    if let Some(s) = json_text(v, &["stock_analysis", "valuation_label"]) {
        lines.push(format!("Valuation: {s}"));
    }
    if let Some(s) = json_text(v, &["research_summary", "suggested_action"]) {
        lines.push(format!("Suggested action: {s}"));
    }
    if let Some(s) = json_text(v, &["research_summary", "final_view"]) {
        lines.push(String::new());
        lines.push("Final view".to_string());
        lines.push(s);
    }
    for (title, path) in [
        ("Business quality", ["research_summary", "business_quality"].as_slice()),
        ("Growth", ["research_summary", "growth"].as_slice()),
        ("Valuation narrative", ["research_summary", "valuation"].as_slice()),
        ("Technical position", ["research_summary", "technical_position"].as_slice()),
        ("Key risks", ["research_summary", "key_risks"].as_slice()),
        ("Stock analysis", ["stock_analysis", "narrative"].as_slice()),
    ] {
        if let Some(s) = json_text(v, path) {
            lines.push(String::new());
            lines.push(title.to_string());
            lines.push(s);
        }
    }
    let positives = json_list(v, &["research_summary", "key_positives"]);
    if !positives.is_empty() {
        lines.push(String::new());
        lines.push("Key positives".to_string());
        for p in positives {
            lines.push(format!("- {p}"));
        }
    }
    let negatives = json_list(v, &["research_summary", "key_negatives"]);
    if !negatives.is_empty() {
        lines.push(String::new());
        lines.push("Key negatives".to_string());
        for p in negatives {
            lines.push(format!("- {p}"));
        }
    }
    lines.push(String::new());
    lines.push("This PDF is a text export of heuristic research support. It is not investment advice.".to_string());
    lines.join("\n")
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        if para.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            if line.is_empty() {
                line = word.to_string();
            } else if line.len() + 1 + word.len() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(line);
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

fn pdf_safe(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '₹' => "Rs".to_string(),
            c if (c as u32) <= 255 => c.to_string(),
            _ => "?".to_string(),
        })
        .collect()
}

fn pdf_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

pub fn report_pdf_bytes(value: &Value) -> Vec<u8> {
    let body = report_plain_text(value);
    lines_to_pdf(&wrap_text(&pdf_safe(&body), 90))
}

fn lines_to_pdf(lines: &[String]) -> Vec<u8> {
    const PAGE_W: i32 = 612;
    const PAGE_H: i32 = 792;
    const MARGIN: i32 = 50;
    const FONT_SIZE: i32 = 10;
    const LEADING: i32 = 13;
    let usable = PAGE_H - MARGIN * 2;
    let per_page = (usable / LEADING).max(1) as usize;
    let chunks: Vec<&[String]> = if lines.is_empty() {
        vec![&[] as &[String]]
    } else {
        lines.chunks(per_page).collect()
    };

    let mut buf = Vec::new();
    let mut offsets = vec![0u32; 1];
    buf.extend(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut start_obj = |buf: &mut Vec<u8>, offsets: &mut Vec<u32>| {
        offsets.push(buf.len() as u32);
        let id = offsets.len() - 1;
        buf.extend(format!("{id} 0 obj\n").as_bytes());
        id
    };
    let end_obj = |buf: &mut Vec<u8>| buf.extend(b"endobj\n");

    let catalog_id = start_obj(&mut buf, &mut offsets);
    buf.extend(b"<< /Type /Catalog /Pages 2 0 R >>\n");
    end_obj(&mut buf);

    let pages_id = start_obj(&mut buf, &mut offsets);
    debug_assert_eq!(pages_id, 2);

    let font_id = 3;
    let first_page_id = 4;
    let mut page_ids = Vec::new();
    let mut content_ids = Vec::new();
    for i in 0..chunks.len() {
        page_ids.push(first_page_id + i * 2);
        content_ids.push(first_page_id + i * 2 + 1);
    }

    let kids: String = page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    buf.extend(
        format!(
            "<< /Type /Pages /Kids [ {kids} ] /Count {} >>\n",
            page_ids.len()
        )
        .as_bytes(),
    );
    end_obj(&mut buf);

    let fid = start_obj(&mut buf, &mut offsets);
    debug_assert_eq!(fid, font_id);
    buf.extend(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\n");
    end_obj(&mut buf);

    for (i, chunk) in chunks.iter().enumerate() {
        let page_id = start_obj(&mut buf, &mut offsets);
        debug_assert_eq!(page_id, page_ids[i]);
        buf.extend(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 {font_id} 0 R >> >> /Contents {} 0 R >>\n",
                content_ids[i]
            )
            .as_bytes(),
        );
        end_obj(&mut buf);

        let mut stream = String::from("BT\n/F1 10 Tf\n");
        stream.push_str(&format!("1 0 0 1 {MARGIN} {} Tm\n", PAGE_H - MARGIN));
        stream.push_str(&format!("{FONT_SIZE} TL\n"));
        for (li, line) in chunk.iter().enumerate() {
            if li > 0 {
                stream.push_str("T*\n");
            }
            stream.push_str(&format!("({}) Tj\n", pdf_escape(line)));
        }
        stream.push_str("ET\n");
        let cid = start_obj(&mut buf, &mut offsets);
        debug_assert_eq!(cid, content_ids[i]);
        buf.extend(format!("<< /Length {} >>\nstream\n", stream.len()).as_bytes());
        buf.extend(stream.as_bytes());
        buf.extend(b"\nendstream\n");
        end_obj(&mut buf);
    }

    let xref_at = buf.len();
    let nobj = offsets.len();
    buf.extend(format!("xref\n0 {nobj}\n").as_bytes());
    buf.extend(b"0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        buf.extend(format!("{off:010} 00000 n \n").as_bytes());
    }
    buf.extend(
        format!("trailer\n<< /Size {nobj} /Root {catalog_id} 0 R >>\nstartxref\n{xref_at}\n%%EOF\n")
            .as_bytes(),
    );
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn xml_escapes_and_nests() {
        let v = json!({"symbol": "RELIANCE.NS", "note": "a < b & c"});
        let xml = value_to_xml("research_report", &v);
        assert!(xml.contains("<symbol>RELIANCE.NS</symbol>"));
        assert!(xml.contains("a &lt; b &amp; c"));
        assert!(xml.starts_with("<?xml"));
    }

    #[test]
    fn json_and_pdf_export_round_trip_shape() {
        let report = json!({
            "symbol": "RELIANCE.NS",
            "long_name": "Reliance Industries",
            "price": 1316.0,
            "report_insights": { "executive_summary": "Test summary" },
            "research_summary": { "suggested_action": "Watch", "key_positives": ["Cash"] }
        });
        let json_bytes = report_export_bytes(&report, ReportExportFormat::Json).unwrap();
        assert!(String::from_utf8_lossy(&json_bytes).contains("RELIANCE.NS"));
        let xml_bytes = report_export_bytes(&report, ReportExportFormat::Xml).unwrap();
        assert!(String::from_utf8_lossy(&xml_bytes).contains("<symbol>RELIANCE.NS</symbol>"));
        let pdf = report_export_bytes(&report, ReportExportFormat::Pdf).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(5).any(|w| w == b"%%EOF"));
        assert!(export_filename("RELIANCE.NS", ReportExportFormat::Pdf).ends_with(".pdf"));
    }
}
