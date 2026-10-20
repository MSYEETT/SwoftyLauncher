use serde_json::{Value, json};

use crate::store::now_ms;

pub fn group(v: i64) -> String {
    let neg = v < 0;
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if neg { format!("-{out}") } else { out }
}

pub fn money(v: i64) -> String {
    if v < 0 {
        format!("-${}", group(-v))
    } else {
        format!("${}", group(v))
    }
}

pub fn money_short(v: i64) -> String {
    let a = v.unsigned_abs() as f64;
    let sign = if v < 0 { "-" } else { "" };
    if a >= 1e9 {
        format!("{sign}${:.2}B", a / 1e9)
    } else if a >= 1e6 {
        format!("{sign}${:.1}M", a / 1e6)
    } else if a >= 1e3 {
        format!("{sign}${:.0}K", a / 1e3)
    } else {
        format!("{sign}${a:.0}")
    }
}

pub fn signed_money_short(v: i64) -> String {
    if v > 0 {
        format!("+{}", money_short(v))
    } else {
        money_short(v)
    }
}

pub fn ago(ms: u64) -> String {
    let d = now_ms().saturating_sub(ms) / 1000;
    match d {
        0..=4 => "just now".into(),
        5..=59 => format!("{d} s ago"),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        _ => format!("{} d ago", d / 86_400),
    }
}

pub fn clock(ms: u64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(ms as i64) {
        chrono::LocalResult::Single(t) => {
            if now_ms().saturating_sub(ms) < 86_400_000 {
                t.format("%H:%M:%S").to_string()
            } else {
                t.format("%d %b %H:%M").to_string()
            }
        }
        _ => "?".into(),
    }
}

pub fn bytes(n: u64) -> String {
    let f = n as f64;
    if f >= 1048576.0 {
        format!("{:.1} MB", f / 1048576.0)
    } else if f >= 1024.0 {
        format!("{:.0} kB", f / 1024.0)
    } else {
        format!("{n} B")
    }
}

pub fn title_case(s: &str) -> String {
    s.split(['_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn track_name(weekend: &str) -> String {
    let t = weekend.strip_prefix("rw_").unwrap_or(weekend);
    let t = t
        .strip_suffix("_26")
        .or_else(|| t.strip_suffix("_25"))
        .unwrap_or(t);
    match t {
        "usa" => "USA".to_owned(),
        "uae" => "UAE".to_owned(),
        _ => title_case(t),
    }
}

pub fn pill(text: &str, tone: &str) -> Value {
    json!({ "text": text, "tone": tone })
}

pub fn dot(text: &str, tone: &str) -> Value {
    json!({ "text": text, "tone": tone, "dot": true })
}

pub fn fact(label: &str, value: impl Into<String>) -> Value {
    json!({ "label": label, "value": value.into() })
}

pub fn text(t: &str, tone: &str) -> Value {
    json!({ "text": { "text": t, "tone": tone } })
}

pub fn card(title: &str, blocks: Vec<Value>) -> Value {
    json!({ "card": { "title": title, "blocks": blocks } })
}

pub fn card_full(
    title: &str,
    subtitle: Option<&str>,
    pills: Vec<Value>,
    buttons: Vec<Value>,
    blocks: Vec<Value>,
) -> Value {
    let mut c = json!({ "title": title, "blocks": blocks });
    if let Some(s) = subtitle {
        c["subtitle"] = json!(s);
    }
    if !pills.is_empty() {
        c["pills"] = json!(pills);
    }
    if !buttons.is_empty() {
        c["buttons"] = json!(buttons);
    }
    json!({ "card": c })
}

pub fn grid(columns: Value, blocks: Vec<Value>) -> Value {
    json!({ "grid": { "columns": columns, "blocks": blocks } })
}

pub fn button(id: &str, label: &str, icon: &str, style: &str) -> Value {
    let mut b = json!({ "id": id, "label": label, "style": style });
    if !icon.is_empty() {
        b["icon"] = json!(icon);
    }
    b
}

pub fn confirm(mut b: Value, title: &str, text: &str) -> Value {
    b["kind"] = json!({ "confirm": { "title": title, "text": text } });
    b
}

pub fn unavailable(title: &str, why: &str) -> Value {
    card(title, vec![text(why, "muted")])
}

pub fn notice(t: &str, tone: &str) -> Value {
    json!({ "text": t, "tone": tone })
}

pub fn int(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f.round() as i64))
}

pub fn short_json(v: &Value) -> String {
    let s = match v {
        Value::Null => "—".to_owned(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if s.chars().count() > 60 {
        format!("{}…", s.chars().take(59).collect::<String>())
    } else {
        s
    }
}
