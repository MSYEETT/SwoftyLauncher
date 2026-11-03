//! Editor support: highlighting tokens, completions and hover docs.

use std::collections::HashSet;
use std::ops::Range;

use crate::config::{self, RawEntry};
use crate::lint::bounds;
use crate::text::{line_at, number};
use crate::{Catalog, Completion, CompletionKind, Effect, Hover, Param, ParamKind, Token, TokenKind, Value};

fn skip_ws(s: &str, mut i: usize) -> usize {
    while s[i..].starts_with([' ', '\t']) {
        i += 1;
    }
    i
}

fn next_char(s: &str, i: usize) -> char {
    s[i..].chars().next().unwrap_or('\0')
}

fn push(out: &mut Vec<Token>, kind: TokenKind, start: usize, range: Range<usize>) {
    if !range.is_empty() {
        out.push(Token {
            kind,
            range: start + range.start..start + range.end,
        });
    }
}

fn scan_values(line: &str, mut i: usize, start: usize, out: &mut Vec<Token>) {
    while i < line.len() {
        i = skip_ws(line, i);
        if i >= line.len() {
            break;
        }
        let c = next_char(line, i);
        match c {
            '#' => {
                push(out, TokenKind::Comment, start, i..line.len());
                break;
            }
            '"' | '\'' => {
                let mut end = line.len();
                let mut escaped = false;
                for (k, d) in line[i + 1..].char_indices() {
                    if c == '"' && escaped {
                        escaped = false;
                    } else if c == '"' && d == '\\' {
                        escaped = true;
                    } else if d == c {
                        end = i + 1 + k + 1;
                        break;
                    }
                }
                push(out, TokenKind::String, start, i..end);
                i = end;
            }
            '[' | ']' | ',' | '{' | '}' | '=' => {
                push(out, TokenKind::Punct, start, i..i + 1);
                i += 1;
            }
            _ => {
                let end = line[i..]
                    .find([' ', '\t', ',', ']', '[', '#', '"', '\'', '='])
                    .map_or(line.len(), |k| i + k)
                    .max(i + c.len_utf8());
                let word = &line[i..end];
                if word == "true" || word == "false" {
                    push(out, TokenKind::Bool, start, i..end);
                } else if word.starts_with(|d: char| d.is_ascii_digit() || matches!(d, '+' | '-' | '.')) {
                    push(out, TokenKind::Number, start, i..end);
                }
                i = end;
            }
        }
    }
}

/// Highlighting tokens; tolerant of broken text.
pub fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        let i = skip_ws(line, 0);
        if i >= line.len() {
            continue;
        }
        if line[i..].starts_with('#') {
            push(&mut out, TokenKind::Comment, start, i..line.len());
        } else if line[i..].starts_with('[') {
            let open = if line[i..].starts_with("[[") { 2 } else { 1 };
            push(&mut out, TokenKind::Punct, start, i..i + open);
            let close = line[i + open..].find(']').map(|k| i + open + k);
            let inner_end = close.unwrap_or_else(|| line[i..].find('#').map_or(line.len(), |k| i + k));
            let inner = &line[i + open..inner_end];
            let lead = inner.len() - inner.trim_start().len();
            let trimmed = inner.trim();
            push(
                &mut out,
                TokenKind::Header,
                start,
                i + open + lead..i + open + lead + trimmed.len(),
            );
            if let Some(c) = close {
                let end = c + if line[c..].starts_with("]]") { 2 } else { 1 };
                push(&mut out, TokenKind::Punct, start, c..end);
                scan_values(line, end, start, &mut out);
            }
        } else {
            let key_end = line[i..]
                .find([' ', '\t', '=', '#'])
                .map_or(line.len(), |k| i + k)
                .max(i + next_char(line, i).len_utf8());
            push(&mut out, TokenKind::Key, start, i..key_end);
            scan_values(line, key_end, start, &mut out);
        }
    }
    out
}

fn in_comment(prefix: &str) -> bool {
    let (mut dq, mut sq, mut escaped) = (false, false, false);
    for c in prefix.chars() {
        if dq && escaped {
            escaped = false;
        } else if dq && c == '\\' {
            escaped = true;
        } else if c == '"' && !sq {
            dq = !dq;
        } else if c == '\'' && !dq {
            sq = !sq;
        } else if c == '#' && !dq && !sq {
            return true;
        }
    }
    false
}

fn header_effect(line: &str) -> Option<String> {
    let t = line.trim_start_matches([' ', '\t']);
    let t = t.strip_prefix('[')?;
    if t.starts_with('[') {
        return None;
    }
    let id: String = t
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    Some(id)
}

fn key_of(line: &str) -> Option<String> {
    let t = line.trim_start_matches([' ', '\t']);
    if t.starts_with(['#', '[']) {
        return None;
    }
    let key: String = t
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let rest = t[key.len()..].trim_start_matches([' ', '\t']);
    (!key.is_empty() && rest.starts_with('=')).then_some(key)
}

/// The effect table the line at `line_start` belongs to (`None` at top level) and the keys that
/// table already sets on other lines.
fn table_context(text: &str, line_start: usize) -> (Option<String>, HashSet<String>) {
    let mut tables: Vec<(Option<String>, HashSet<String>)> = vec![(None, HashSet::new())];
    let mut found = None;
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        if let Some(id) = header_effect(line) {
            tables.push((Some(id), HashSet::new()));
            if start == line_start {
                found = Some(tables.len() - 1);
            }
        } else if start == line_start {
            found = Some(tables.len() - 1);
        } else if let Some(k) = key_of(line) {
            tables.last_mut().expect("never empty").1.insert(k);
        }
    }
    tables.swap_remove(found.unwrap_or(tables.len() - 1))
}

fn word_end(text: &str, from: usize) -> usize {
    let rest = &text[from..];
    from + rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(rest.len())
}

fn type_name(kind: &ParamKind) -> &'static str {
    match kind {
        ParamKind::Float { .. } => "float",
        ParamKind::Int { .. } => "int",
        ParamKind::Bool => "bool",
        ParamKind::Enum { .. } => "enum",
        ParamKind::Float2 { .. } => "float2",
        ParamKind::Color3 { .. } => "color3",
        ParamKind::Color4 { .. } => "color4",
    }
}

fn default_text(p: &Param) -> String {
    let float = !matches!(p.kind, ParamKind::Int { .. });
    match &p.default {
        Value::Number(n) => number(*n, float),
        Value::Bool(b) => b.to_string(),
        Value::Text(t) => format!("\"{t}\""),
        Value::Vector(v) => format!(
            "[{}]",
            v.iter().map(|n| number(*n, true)).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn range_text(p: &Param) -> Option<String> {
    let float = !matches!(p.kind, ParamKind::Int { .. });
    match bounds(&p.kind) {
        (Some(a), Some(b)) => Some(format!("{} to {}", number(a, float), number(b, float))),
        (Some(a), None) => Some(format!("at least {}", number(a, float))),
        (None, Some(b)) => Some(format!("at most {}", number(b, float))),
        (None, None) => None,
    }
}

fn key_completions(
    catalog: &Catalog,
    effect: Option<&str>,
    set: &HashSet<String>,
    typed: &str,
    range: Range<usize>,
    full_line: bool,
) -> Vec<Completion> {
    let typed = typed.to_ascii_lowercase();
    let wanted = |k: &str| !set.contains(k) && k.to_ascii_lowercase().starts_with(&typed);
    let make = |label: &str, detail: String, doc: String, default: String| Completion {
        label: label.to_owned(),
        kind: if effect.is_some() {
            CompletionKind::Param
        } else {
            CompletionKind::Keyword
        },
        detail,
        doc,
        range: range.clone(),
        insert: if full_line {
            format!("{label} = {default}")
        } else {
            label.to_owned()
        },
    };
    match effect {
        None => [
            ("name", "The config's name", "\"\""),
            ("author", "Who made the config", "\"\""),
            ("description", "A short description", "\"\""),
        ]
        .into_iter()
        .filter(|(k, ..)| wanted(k))
        .map(|(k, doc, d)| make(k, "string".into(), doc.into(), d.into()))
        .collect(),
        Some(id) => {
            let Some(e) = catalog.get(id) else {
                return Vec::new();
            };
            let mut out: Vec<Completion> = e
                .params
                .iter()
                .filter(|p| wanted(&p.id))
                .map(|p| {
                    let detail = format!("{} · default {}", p.label, default_text(p));
                    make(&p.id, detail, p.doc.clone(), default_text(p))
                })
                .collect();
            if wanted("enabled") {
                out.push(make(
                    "enabled",
                    "bool · default true".into(),
                    "Turn this instance off without deleting it.".into(),
                    "false".into(),
                ));
            }
            out
        }
    }
}

fn value_completions(p: &Param, range: Range<usize>, typed: &str) -> Vec<Completion> {
    let item = |label: String, insert: String| Completion {
        label,
        kind: CompletionKind::Value,
        detail: p.label.clone(),
        doc: p.doc.clone(),
        range: range.clone(),
        insert,
    };
    let typed = typed.trim_matches('"').to_ascii_lowercase();
    match &p.kind {
        ParamKind::Enum { values } => values
            .iter()
            .filter(|v| v.to_ascii_lowercase().starts_with(&typed))
            .map(|v| item(v.clone(), format!("\"{v}\"")))
            .collect(),
        ParamKind::Bool => ["true", "false"]
            .into_iter()
            .filter(|v| v.starts_with(&typed))
            .map(|v| item(v.to_owned(), v.to_owned()))
            .collect(),
        _ => {
            let d = default_text(p);
            vec![item(d.clone(), d)]
        }
    }
}

pub fn completions(text: &str, offset: usize, catalog: &Catalog) -> Vec<Completion> {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let line = line_at(text, offset);
    let prefix = &text[line.start..offset];
    let comment = in_comment(prefix);
    if comment {
        return Vec::new();
    }
    let lead = prefix.len() - prefix.trim_start_matches([' ', '\t']).len();
    let body = &prefix[lead..];

    if let Some(after) = body.strip_prefix('[') {
        if after.starts_with('[') || body.contains([']', '.']) {
            return Vec::new();
        }
        let typed = after.trim_start();
        let from = offset - typed.len();
        let end = word_end(text, offset);
        let closed = text[end..line.end].trim_start().starts_with(']');
        let t = typed.to_ascii_lowercase();
        return catalog
            .effects
            .iter()
            .filter(|e| e.id.starts_with(&t))
            .map(|e| Completion {
                label: e.id.clone(),
                kind: CompletionKind::Effect,
                detail: e.name.clone(),
                doc: e.description.clone(),
                range: from..end,
                insert: if closed {
                    e.id.clone()
                } else {
                    format!("{}]", e.id)
                },
            })
            .collect();
    }

    if let Some(eq) = body.find('=')
        && !body[..eq].contains(['"', '\''])
    {
        let key = body[..eq].trim();
        let abs_eq = line.start + lead + eq;
        let after = &text[abs_eq + 1..];
        let vstart = (abs_eq + 1 + after.len() - after.trim_start_matches([' ', '\t']).len()).min(offset);
        let (effect, _) = table_context(text, line.start);
        let Some(p) = effect.and_then(|id| catalog.get(&id)?.params.iter().find(|p| p.id == key)) else {
            return Vec::new();
        };
        if text[vstart..offset].starts_with('[') {
            return Vec::new();
        }
        let quoted = text[vstart..].starts_with('"');
        let end = if quoted {
            let tail = &text[vstart + 1..line.end];
            tail.find('"')
                .map_or(line.end, |k| vstart + 1 + k + 1)
                .max(offset)
        } else {
            let tail = &text[offset..line.end];
            offset + tail.find([' ', '\t', ',', ']', '#']).unwrap_or(tail.len())
        };
        return value_completions(p, vstart..end, &text[vstart..offset]);
    }

    if body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        let from = offset - body.len();
        let end = word_end(text, offset);
        let full_line = !text[end..line.end].trim_start().starts_with('=');
        let (effect, set) = table_context(text, line.start);
        let in_table = text[..line.start].lines().any(|l| header_effect(l).is_some());
        if effect.is_none() && in_table {
            return Vec::new();
        }
        return key_completions(catalog, effect.as_deref(), &set, body, from..end, full_line);
    }
    Vec::new()
}

fn effect_hover(e: &Effect, range: Range<usize>) -> Hover {
    let mut text = format!("**{}** (`{}`)", e.name, e.id);
    if !e.description.is_empty() {
        text.push_str(&format!("\n\n{}", e.description));
    }
    if !e.params.is_empty() {
        text.push_str("\n\n");
        let lines: Vec<String> = e
            .params
            .iter()
            .map(|p| format!("- `{}` {} ({})", p.id, p.label, type_name(&p.kind)))
            .collect();
        text.push_str(&lines.join("\n"));
    }
    Hover { text, range }
}

fn param_hover(p: &Param, range: Range<usize>) -> Hover {
    let mut text = format!("**{}** (`{}`, {})", p.label, p.id, type_name(&p.kind));
    if !p.doc.is_empty() {
        text.push_str(&format!("\n\n{}", p.doc));
    }
    if let ParamKind::Enum { values } = &p.kind {
        text.push_str(&format!(
            "\n\nOne of: {}",
            values
                .iter()
                .map(|v| format!("`{v}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    } else if let Some(r) = range_text(p) {
        text.push_str(&format!("\n\nRange: {r}"));
    }
    text.push_str(&format!("\n\nDefault: `{}`", default_text(p)));
    Hover { text, range }
}

fn entry_hover(e: &RawEntry, offset: usize, effect: Option<&Effect>) -> Option<Hover> {
    let range = if e.key_range.contains(&offset) || e.key_range.end == offset {
        e.key_range.clone()
    } else {
        e.value.range.clone()
    };
    match effect {
        Some(_) if e.key == "enabled" => Some(Hover {
            text: "**Enabled** (`enabled`, bool)\n\nTurn this instance off without deleting it.\n\nDefault: `true`".into(),
            range,
        }),
        Some(fx) => fx.params.iter().find(|p| p.id == e.key).map(|p| param_hover(p, range)),
        None => {
            let doc = match e.key.as_str() {
                "name" => "The config's name, shown in the list.",
                "author" => "Who made the config.",
                "description" => "A short description of the look.",
                "swfx" => "The config format version; only 1 exists.",
                _ => return None,
            };
            Some(Hover { text: format!("**{}** (`{}`)\n\n{}", e.key, e.key, doc), range })
        }
    }
}

pub fn hover(text: &str, offset: usize, catalog: &Catalog) -> Option<Hover> {
    let doc = config::parse(text);
    if let Some(t) = doc
        .tables
        .iter()
        .find(|t| t.header.start <= offset && offset <= t.header.end)
    {
        let e = catalog.get(&t.effect)?;
        return Some(effect_hover(e, t.effect_range.clone()));
    }
    for e in &doc.top {
        if e.range.start <= offset && offset <= e.range.end {
            return entry_hover(e, offset, None);
        }
    }
    for t in doc.tables.iter().filter(|t| !t.bad) {
        for e in &t.entries {
            if e.range.start <= offset && offset <= e.range.end {
                return entry_hover(e, offset, catalog.get(&t.effect));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::tests::{GOOD, catalog};

    fn at(text: &str) -> (String, usize) {
        let i = text.find('|').expect("cursor");
        (text.replacen('|', "", 1), i)
    }

    fn comp(text: &str) -> (String, Vec<Completion>) {
        let (t, i) = at(text);
        let c = completions(&t, i, &catalog());
        (t, c)
    }

    fn labels(c: &[Completion]) -> Vec<&str> {
        c.iter().map(|c| c.label.as_str()).collect()
    }

    fn kinds(text: &str) -> Vec<(TokenKind, String)> {
        tokens(text)
            .into_iter()
            .map(|t| (t.kind, text[t.range].to_owned()))
            .collect()
    }

    #[test]
    fn tokens_cover_every_construct() {
        let k = kinds("# c\nname = \"A b\" # t\n[grain.x]\namount = [1, -2.5e3]\non = true\n");
        use TokenKind::*;
        let want = [
            (Comment, "# c"),
            (Key, "name"),
            (Punct, "="),
            (String, "\"A b\""),
            (Comment, "# t"),
            (Punct, "["),
            (Header, "grain.x"),
            (Punct, "]"),
            (Key, "amount"),
            (Punct, "="),
            (Punct, "["),
            (Number, "1"),
            (Punct, ","),
            (Number, "-2.5e3"),
            (Punct, "]"),
            (Key, "on"),
            (Punct, "="),
            (Bool, "true"),
        ];
        assert_eq!(k.len(), want.len(), "{k:?}");
        for (g, w) in k.iter().zip(want) {
            assert_eq!((g.0, g.1.as_str()), w);
        }
    }

    #[test]
    fn tokens_handle_broken_text() {
        let k = kinds("[grain\nx = \"open\ny 'z\n");
        assert!(k.contains(&(TokenKind::Header, "grain".into())));
        assert!(k.contains(&(TokenKind::String, "\"open".into())));
        let k = kinds("a = \"q\\\"r\"");
        assert!(k.contains(&(TokenKind::String, "\"q\\\"r\"".into())));
    }

    #[test]
    fn nothing_panics_on_any_prefix_of_any_config() {
        let cat = catalog();
        let texts = [
            GOOD,
            "é😀 = [1,\n[[x]]\n[a.b.c]\n\"\"\"x\n'''y\n= = =\n\u{feff}[x]\r\n#é",
            crate::effect::tests::AMBIENT,
        ];
        for text in texts {
            for (i, _) in text.char_indices().chain([(text.len(), ' ')]) {
                let p = &text[..i];
                let _ = tokens(p);
                let _ = crate::lint(p, &cat);
                let _ = completions(p, p.len(), &cat);
                let _ = hover(p, p.len().saturating_sub(1), &cat);
                for j in 0..=p.len() {
                    let _ = completions(p, j, &cat);
                    let _ = hover(p, j, &cat);
                }
            }
        }
    }

    #[test]
    fn effect_ids_complete_after_a_bracket() {
        let (t, c) = comp("name = \"a\"\n[|");
        assert_eq!(labels(&c), ["ambient_light", "grain", "bloom_wide"]);
        assert_eq!(c[0].detail, "Ambient Light");
        assert_eq!(c[0].insert, "ambient_light]");
        assert_eq!(c[0].range, t.len()..t.len());
    }

    #[test]
    fn a_partial_effect_word_is_replaced_whole() {
        let (t, c) = comp("[gr|ain]\n");
        assert_eq!(labels(&c), ["grain"]);
        assert_eq!(&t[c[0].range.clone()], "grain");
        assert_eq!(c[0].insert, "grain");
        let (t, c) = comp("[am|");
        assert_eq!(&t[c[0].range.clone()], "am");
        assert_eq!(c[0].insert, "ambient_light]");
        assert!(comp("[grain.|").1.is_empty());
    }

    #[test]
    fn unset_params_and_enabled_complete_inside_a_table() {
        let (t, c) = comp("[grain]\namount = 1\n|");
        assert_eq!(labels(&c), ["size", "enabled"]);
        assert_eq!(c[0].insert, "size = 2");
        assert_eq!(c[0].range, t.len()..t.len());
        assert_eq!(c[1].insert, "enabled = false");
        let (_, c) = comp("[ambient_light]\n|");
        assert_eq!(c[0].detail, "Bloom intensity · default 1.0");
        assert_eq!(c[0].doc, "How bright the bloom is.");
        assert_eq!(c[0].insert, "intensity = 1.0");
        assert_eq!(
            c.iter().find(|c| c.label == "tint").unwrap().insert,
            "tint = [1.0, 1.0, 1.0]"
        );
    }

    #[test]
    fn a_typed_key_prefix_narrows_and_keeps_the_rest_of_the_line() {
        let (t, c) = comp("[ambient_light]\nin|\n");
        assert_eq!(labels(&c), ["intensity"]);
        assert_eq!(&t[c[0].range.clone()], "in");
        let (t, c) = comp("[grain]\nam|ount = 5\n");
        assert_eq!(labels(&c), ["amount"]);
        assert_eq!(&t[c[0].range.clone()], "amount");
        assert_eq!(c[0].insert, "amount");
    }

    #[test]
    fn the_top_level_offers_metadata_keys() {
        let (_, c) = comp("name = \"a\"\n|");
        assert_eq!(labels(&c), ["author", "description"]);
        assert!(comp("[grain]\n#|").1.is_empty());
    }

    #[test]
    fn values_complete_after_equals() {
        let (t, c) = comp("[ambient_light]\nadapt_black_level = |\n");
        assert_eq!(labels(&c), ["off", "low", "high"]);
        assert_eq!(c[1].insert, "\"low\"");
        assert_eq!(c[0].range, t.len() - 1..t.len() - 1);
        let (t, c) = comp("[ambient_light]\nadapt_black_level = \"lo|\"\n");
        assert_eq!(labels(&c), ["low"]);
        assert_eq!(&t[c[0].range.clone()], "\"lo\"");
        let (_, c) = comp("[ambient_light]\nlens = |\n");
        assert_eq!(labels(&c), ["true", "false"]);
        let (_, c) = comp("[ambient_light]\nlens = t|\n");
        assert_eq!(labels(&c), ["true"]);
        let (_, c) = comp("[grain]\namount = |\n");
        assert_eq!(c[0].insert, "0.1");
        let (_, c) = comp("[grain]\nsize = |\n");
        assert_eq!(c[0].insert, "2");
        let (_, c) = comp("[ambient_light]\ntint = |\n");
        assert_eq!(c[0].insert, "[1.0, 1.0, 1.0]");
        assert!(comp("[ambient_light]\nnope = |\n").1.is_empty());
        assert!(comp("name = |\n").1.is_empty());
    }

    #[test]
    fn hover_describes_effects_and_params() {
        let cat = catalog();
        let t = "name = \"a\"\n[ambient_light]\nintensity = 1.5\nenabled = true\n";
        let h = hover(t, t.find("ambient").unwrap() + 2, &cat).unwrap();
        assert!(h.text.starts_with("**Ambient Light** (`ambient_light`)"));
        assert!(
            h.text.contains("Bloom from bright areas.")
                && h.text.contains("- `intensity` Bloom intensity (float)")
        );
        assert_eq!(&t[h.range], "ambient_light");
        let h = hover(t, t.find("intensity").unwrap() + 1, &cat).unwrap();
        assert!(
            h.text.contains("**Bloom intensity**")
                && h.text.contains("Range: 0.0 to 4.0")
                && h.text.contains("Default: `1.0`")
        );
        assert_eq!(&t[h.range], "intensity");
        let h = hover(t, t.find("1.5").unwrap(), &cat).unwrap();
        assert_eq!(&t[h.range.clone()], "1.5");
        assert!(
            hover(t, t.find("enabled").unwrap(), &cat)
                .unwrap()
                .text
                .contains("Turn this instance off")
        );
        assert!(
            hover(t, t.find("name").unwrap(), &cat)
                .unwrap()
                .text
                .contains("shown in the list")
        );
        assert!(hover(t, 0, &cat).is_some());
        let e = "[ambient_light]\nadapt_black_level = \"low\"\n";
        assert!(
            hover(e, e.find("low").unwrap(), &cat)
                .unwrap()
                .text
                .contains("One of: `off`, `low`, `high`")
        );
        assert!(hover("[nope]\nx = 1\n", 2, &cat).is_none());
        assert!(hover("# c\n", 1, &cat).is_none());
    }
}
