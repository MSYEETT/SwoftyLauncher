//! The tolerant `.swfx` parser: the TOML subset a shader config uses, one line at a time.
//!
//! Every problem becomes an error in [`RawDoc::errors`] and parsing goes on with the next line, so
//! one text can report several.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Lit {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<Elem>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Elem {
    pub(crate) num: f64,
    pub(crate) int: bool,
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawValue {
    pub(crate) lit: Lit,
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawEntry {
    pub(crate) key: String,
    pub(crate) key_range: Range<usize>,
    pub(crate) value: RawValue,
    /// The whole line without indentation or comment.
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawTable {
    pub(crate) effect: String,
    pub(crate) effect_range: Range<usize>,
    pub(crate) label: Option<String>,
    pub(crate) header: Range<usize>,
    pub(crate) entries: Vec<RawEntry>,
    pub(crate) span: Range<usize>,
    pub(crate) bad: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawError {
    pub(crate) range: Range<usize>,
    pub(crate) message: String,
}

#[derive(Debug, Default)]
pub(crate) struct RawDoc {
    pub(crate) top: Vec<RawEntry>,
    pub(crate) tables: Vec<RawTable>,
    pub(crate) errors: Vec<RawError>,
}

fn bare_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn digits(s: &str) -> Option<&str> {
    let mut prev_us = true;
    let mut any = false;
    for c in s.chars() {
        match c {
            '0'..='9' => {
                prev_us = false;
                any = true;
            }
            '_' if !prev_us => prev_us = true,
            _ => return None,
        }
    }
    (any && !prev_us).then_some(s)
}

/// TOML's integer and float grammar. Returns the value and whether it is an integer.
pub(crate) fn parse_number(tok: &str) -> Option<(f64, bool)> {
    let body = tok.strip_prefix(['+', '-']).unwrap_or(tok);
    let (int_part, rest) = match body.find(['.', 'e', 'E']) {
        Some(i) => (&body[..i], &body[i..]),
        None => (body, ""),
    };
    digits(int_part)?;
    if int_part.len() > 1 && int_part.starts_with('0') {
        return None;
    }
    let clean: String = tok.chars().filter(|&c| c != '_').collect();
    if rest.is_empty() {
        return clean.parse::<i64>().ok().map(|v| (v as f64, true));
    }
    let (frac, exp) = match rest.strip_prefix('.') {
        Some(r) => match r.find(['e', 'E']) {
            Some(i) => (Some(&r[..i]), &r[i..]),
            None => (Some(r), ""),
        },
        None => (None, rest),
    };
    if let Some(f) = frac {
        digits(f)?;
    }
    if !exp.is_empty() {
        digits(exp[1..].strip_prefix(['+', '-']).unwrap_or(&exp[1..]))?;
    }
    clean
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| (v, false))
}

struct LineCtx<'a> {
    line: &'a str,
    start: usize,
}

impl LineCtx<'_> {
    fn at(&self, from: usize, to: usize) -> Range<usize> {
        self.start + from..self.start + to
    }
}

fn skip_ws(s: &str, mut i: usize) -> usize {
    while s[i..].starts_with([' ', '\t']) {
        i += 1;
    }
    i
}

fn string_basic(ctx: &LineCtx, open: usize) -> Result<(String, usize), RawError> {
    let line = ctx.line;
    let mut out = String::new();
    let mut chars = line[open + 1..].char_indices();
    while let Some((k, c)) = chars.next() {
        let at = open + 1 + k;
        match c {
            '"' => return Ok((out, at + 1)),
            '\\' => {
                let Some((_, e)) = chars.next() else { break };
                match e {
                    'b' => out.push('\u{8}'),
                    't' => out.push('\t'),
                    'n' => out.push('\n'),
                    'f' => out.push('\u{c}'),
                    'r' => out.push('\r'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'u' | 'U' => {
                        let n = if e == 'u' { 4 } else { 8 };
                        let hex: String = chars.by_ref().take(n).map(|(_, c)| c).collect();
                        match u32::from_str_radix(&hex, 16)
                            .ok()
                            .filter(|_| hex.len() == n)
                            .and_then(char::from_u32)
                        {
                            Some(c) => out.push(c),
                            None => {
                                return Err(RawError {
                                    range: ctx.at(at, (at + 2 + hex.len()).min(line.len())),
                                    message: "Invalid unicode escape in string".into(),
                                });
                            }
                        }
                    }
                    other => {
                        return Err(RawError {
                            range: ctx.at(at, at + 1 + other.len_utf8()),
                            message: format!("Unknown escape `\\{other}` in string"),
                        });
                    }
                }
            }
            _ => out.push(c),
        }
    }
    Err(RawError {
        range: ctx.at(open, line.len()),
        message: "This string is not closed on its line".into(),
    })
}

fn number_token(s: &str, from: usize) -> usize {
    s[from..]
        .find([' ', '\t', ',', ']', '#'])
        .map_or(s.len(), |i| from + i)
}

fn bad_token(ctx: &LineCtx, from: usize, to: usize) -> RawError {
    let tok = &ctx.line[from..to];
    let message = if tok.starts_with(|c: char| c.is_ascii_digit()) && tok.contains(['-', ':']) {
        format!("Dates and times (`{tok}`) are not supported in a shader config")
    } else if matches!(tok.trim_start_matches(['+', '-']), "inf" | "nan")
        || tok.starts_with("0x")
        || tok.starts_with("0o")
        || tok.starts_with("0b")
    {
        format!("`{tok}` is not supported in a shader config; write a decimal number")
    } else if tok.is_empty() {
        "Expected a value".to_owned()
    } else {
        format!("`{tok}` is not a valid value; use a number, true, false or a quoted string")
    };
    RawError {
        range: ctx.at(from, to.max(from)),
        message,
    }
}

fn value(ctx: &LineCtx, from: usize) -> Result<(RawValue, usize), RawError> {
    let line = ctx.line;
    let i = skip_ws(line, from);
    let rest = &line[i..];
    let unsupported = |end: usize, what: &str| RawError {
        range: ctx.at(i, end),
        message: format!("{what} is not supported in a shader config"),
    };
    if rest.starts_with("\"\"\"") || rest.starts_with("'''") {
        return Err(unsupported(line.len(), "A multi-line string"));
    }
    if rest.starts_with('{') {
        return Err(unsupported(line.len(), "An inline table"));
    }
    if rest.starts_with('"') {
        let (s, end) = string_basic(ctx, i)?;
        return Ok((
            RawValue {
                lit: Lit::Str(s),
                range: ctx.at(i, end),
            },
            end,
        ));
    }
    if let Some(inner) = rest.strip_prefix('\'') {
        return match inner.find('\'') {
            Some(k) => {
                let end = i + 1 + k + 1;
                Ok((
                    RawValue {
                        lit: Lit::Str(line[i + 1..end - 1].to_owned()),
                        range: ctx.at(i, end),
                    },
                    end,
                ))
            }
            None => Err(RawError {
                range: ctx.at(i, line.len()),
                message: "This string is not closed on its line".into(),
            }),
        };
    }
    if rest.starts_with('[') {
        return array(ctx, i);
    }
    let end = number_token(line, i);
    let tok = &line[i..end];
    match tok {
        "true" => {
            return Ok((
                RawValue {
                    lit: Lit::Bool(true),
                    range: ctx.at(i, end),
                },
                end,
            ));
        }
        "false" => {
            return Ok((
                RawValue {
                    lit: Lit::Bool(false),
                    range: ctx.at(i, end),
                },
                end,
            ));
        }
        _ => {}
    }
    match parse_number(tok) {
        Some((v, true)) => Ok((
            RawValue {
                lit: Lit::Int(v as i64),
                range: ctx.at(i, end),
            },
            end,
        )),
        Some((v, false)) => Ok((
            RawValue {
                lit: Lit::Float(v),
                range: ctx.at(i, end),
            },
            end,
        )),
        None => Err(bad_token(ctx, i, end)),
    }
}

fn array(ctx: &LineCtx, open: usize) -> Result<(RawValue, usize), RawError> {
    let line = ctx.line;
    let mut i = open + 1;
    let mut elems = Vec::new();
    loop {
        i = skip_ws(line, i);
        if i >= line.len() || line[i..].starts_with('#') {
            return Err(RawError {
                range: ctx.at(open, line.len()),
                message: "A multi-line array is not supported in a shader config; keep the array on one line"
                    .into(),
            });
        }
        if line[i..].starts_with(']') {
            return Ok((
                RawValue {
                    lit: Lit::Array(elems),
                    range: ctx.at(open, i + 1),
                },
                i + 1,
            ));
        }
        let end = number_token(line, i);
        match parse_number(&line[i..end]) {
            Some((num, int)) => elems.push(Elem {
                num,
                int,
                range: ctx.at(i, end),
            }),
            None => {
                let mut e = bad_token(ctx, i, end);
                if line[i..].starts_with(['"', '\'', '[', '{']) || matches!(&line[i..end], "true" | "false") {
                    e.range = ctx.at(i, end.max(i + 1));
                    e.message = "An array in a shader config holds numbers only".into();
                }
                return Err(e);
            }
        }
        i = skip_ws(line, end);
        if line[i..].starts_with(',') {
            i += 1;
        } else if !line[i..].starts_with(']') {
            let stop = number_token(line, i).max(i + 1).min(line.len());
            return Err(RawError {
                range: ctx.at(i.min(line.len()), stop),
                message: "Expected `,` or `]` in the array".into(),
            });
        }
    }
}

fn trailing(ctx: &LineCtx, from: usize, errors: &mut Vec<RawError>) {
    let i = skip_ws(ctx.line, from);
    if i < ctx.line.len() && !ctx.line[i..].starts_with('#') {
        errors.push(RawError {
            range: ctx.at(i, ctx.line.len()),
            message: "Unexpected text after the value".into(),
        });
    }
}

fn header(ctx: &LineCtx, at: usize, doc: &mut RawDoc) {
    let line = ctx.line;
    let mk = |bad: bool, effect: String, effect_range: Range<usize>, label: Option<String>, close: usize| {
        RawTable {
            effect,
            effect_range,
            label,
            header: ctx.at(at, close),
            entries: Vec::new(),
            span: ctx.at(at, close),
            bad,
        }
    };
    let Some(close) = line[at..].find(']').map(|i| at + i) else {
        doc.errors.push(RawError {
            range: ctx.at(at, line.len()),
            message: "This table header is missing its `]`".into(),
        });
        doc.tables
            .push(mk(true, String::new(), ctx.at(at, at), None, line.len()));
        return;
    };
    let mut errors = Vec::new();
    trailing(ctx, close + 1, &mut errors);
    doc.errors.append(&mut errors);
    let inner = &line[at + 1..close];
    let mut segments = Vec::new();
    let mut off = at + 1;
    for part in inner.split('.') {
        let lead = part.len() - part.trim_start_matches([' ', '\t']).len();
        let trimmed = part.trim_matches([' ', '\t']);
        segments.push((trimmed, off + lead..off + lead + trimmed.len()));
        off += part.len() + 1;
    }
    let mut bad = false;
    let whole = ctx.at(at, close + 1);
    if inner.contains(['"', '\'']) {
        doc.errors.push(RawError {
            range: whole.clone(),
            message: "A quoted table name is not supported in a shader config".into(),
        });
        bad = true;
    } else if segments.len() > 2 {
        doc.errors.push(RawError {
            range: whole.clone(),
            message: "A shader table is `[effect]` or `[effect.label]`".into(),
        });
        bad = true;
    } else if segments.iter().any(|(s, _)| !bare_key(s)) {
        doc.errors.push(RawError {
            range: whole.clone(),
            message: "A table name is letters, digits, `_` and `-`".into(),
        });
        bad = true;
    }
    let (effect, er) = segments[0].clone();
    let label = segments.get(1).map(|(s, _)| (*s).to_owned()).filter(|_| !bad);
    let mut t = mk(bad, effect.to_owned(), ctx.at(er.start, er.end), label, close + 1);
    t.span = whole;
    doc.tables.push(t);
}

fn entry(ctx: &LineCtx, at: usize, doc: &mut RawDoc) {
    let line = ctx.line;
    let Some(eq) = line[at..].find('=').map(|i| at + i) else {
        doc.errors.push(RawError {
            range: ctx.at(at, line.trim_end().len()),
            message: "Expected `key = value`".into(),
        });
        return;
    };
    let raw_key = &line[at..eq];
    let key = raw_key.trim_end_matches([' ', '\t']);
    let key_range = ctx.at(at, at + key.len());
    if key.is_empty() {
        doc.errors.push(RawError {
            range: ctx.at(eq, eq + 1),
            message: "Missing the key before `=`".into(),
        });
        return;
    }
    if key.contains('"') || key.contains('\'') {
        doc.errors.push(RawError {
            range: key_range,
            message: "A quoted key is not supported in a shader config".into(),
        });
        return;
    }
    if key.contains('.') {
        doc.errors.push(RawError {
            range: key_range,
            message: "A dotted key is not supported in a shader config".into(),
        });
        return;
    }
    if !bare_key(key) {
        doc.errors.push(RawError {
            range: key_range,
            message: format!("`{key}` is not a valid key"),
        });
        return;
    }
    match value(ctx, eq + 1) {
        Ok((v, end)) => {
            let mut errors = Vec::new();
            trailing(ctx, end, &mut errors);
            doc.errors.append(&mut errors);
            let range = ctx.at(at, end);
            let e = RawEntry {
                key: key.to_owned(),
                key_range: key_range.clone(),
                value: v,
                range: range.clone(),
            };
            let target = match doc.tables.last_mut() {
                Some(t) => {
                    t.span.end = range.end;
                    &mut t.entries
                }
                None => &mut doc.top,
            };
            if target.iter().any(|x| x.key == e.key) {
                doc.errors.push(RawError {
                    range: key_range,
                    message: format!("`{key}` is set twice"),
                });
            } else {
                target.push(e);
            }
        }
        Err(e) => doc.errors.push(e),
    }
}

pub(crate) fn parse(text: &str) -> RawDoc {
    let mut doc = RawDoc::default();
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        let line = raw.trim_end_matches(['\n', '\r']);
        let at = skip_ws(line, 0);
        let ctx = LineCtx { line, start };
        let body = &line[at..];
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        if body.starts_with("[[") {
            doc.errors.push(RawError {
                range: ctx.at(at, line.trim_end().len()),
                message: "An array of tables (`[[x]]`) is not supported in a shader config".into(),
            });
            doc.tables.push(RawTable {
                effect: String::new(),
                effect_range: ctx.at(at, at),
                label: None,
                header: ctx.at(at, line.len()),
                entries: Vec::new(),
                span: ctx.at(at, line.len()),
                bad: true,
            });
        } else if body.starts_with('[') {
            header(&ctx, at, &mut doc);
        } else {
            entry(&ctx, at, &mut doc);
        }
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errs(text: &str) -> Vec<String> {
        parse(text).errors.into_iter().map(|e| e.message).collect()
    }

    #[test]
    fn numbers_follow_the_toml_grammar() {
        for ok in [
            "0", "1", "-3", "+4", "1_000", "0.5", "-0.25", "1e3", "1.5E-2", "6.02e+23", "1_0.2_5", "-0",
        ] {
            assert!(parse_number(ok).is_some(), "{ok}");
            ok.parse::<toml::Value>().ok();
        }
        for bad in [
            "", "01", "1__0", "_1", "1_", ".5", "5.", "1e", "1.e3", "0x10", "inf", "nan", "--1", "1.2.3",
            "1e+",
        ] {
            assert!(parse_number(bad).is_none(), "{bad}");
        }
        assert_eq!(parse_number("12"), Some((12.0, true)));
        assert_eq!(parse_number("1e3"), Some((1000.0, false)));
        assert_eq!(parse_number("1_000.5"), Some((1000.5, false)));
    }

    #[test]
    fn a_good_file_parses_to_tables_and_entries() {
        let text = "# hi\nname = \"A\"\n\n[bloom]\nintensity = 1.5 # c\ntint = [1, 2.0, 3,]\non = true\n[bloom.wide]\nx = 'lit'\n";
        let d = parse(text);
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.top.len(), 1);
        assert_eq!(d.tables.len(), 2);
        assert_eq!(d.tables[0].entries.len(), 3);
        assert_eq!(d.tables[1].label.as_deref(), Some("wide"));
        let e = &d.tables[0].entries[0];
        assert_eq!(&text[e.key_range.clone()], "intensity");
        assert_eq!(&text[e.value.range.clone()], "1.5");
        assert_eq!(
            &text[d.tables[0].span.clone()],
            "[bloom]\nintensity = 1.5 # c\ntint = [1, 2.0, 3,]\non = true"
        );
        match &d.tables[0].entries[1].value.lit {
            Lit::Array(a) => assert_eq!(a.len(), 3),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn strings_decode_escapes() {
        let d = parse("name = \"a\\tb\\u00e9\\\"q\\\\\"\n");
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.top[0].value.lit, Lit::Str("a\tbé\"q\\".into()));
        assert!(errs("n = \"a\\qb\"").iter().any(|m| m.contains("Unknown escape")));
        assert!(errs("n = \"abc").iter().any(|m| m.contains("not closed")));
        assert!(errs("n = \"a\\u12\"").iter().any(|m| m.contains("unicode")));
    }

    #[test]
    fn unsupported_toml_is_named() {
        assert!(errs("n = \"\"\"x\"\"\"")[0].contains("multi-line string"));
        assert!(errs("n = '''x'''")[0].contains("multi-line string"));
        assert!(errs("n = {a = 1}")[0].contains("inline table"));
        assert!(errs("n = 1979-05-27")[0].contains("Dates"));
        assert!(errs("n = [1,\n 2]")[0].contains("multi-line array"));
        assert!(errs("[[x]]")[0].contains("array of tables"));
        assert!(errs("a.b = 1")[0].contains("dotted key"));
        assert!(errs("\"a\" = 1")[0].contains("quoted key"));
        assert!(errs("[a.b.c]")[0].contains("[effect.label]"));
        assert!(errs("[\"a\"]")[0].contains("quoted table"));
        assert!(errs("n = [\"a\"]")[0].contains("numbers only"));
        assert!(errs("n = yes")[0].contains("not a valid value"));
        assert!(errs("n = 0x10")[0].contains("decimal"));
    }

    #[test]
    fn it_keeps_going_after_a_bad_line() {
        let d = parse("a = \nb = 2\nc 3\nd = 4 5\n[x\ne = 1\n");
        let msgs: Vec<_> = d.errors.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(msgs.len(), 4, "{msgs:?}");
        assert!(d.top.iter().any(|e| e.key == "b"));
        assert!(d.top.iter().any(|e| e.key == "d"));
        assert!(d.tables[0].bad);
    }

    #[test]
    fn duplicate_keys_are_errors_in_their_table() {
        assert!(errs("a = 1\na = 2")[0].contains("set twice"));
        assert!(errs("[x]\na = 1\n[y]\na = 2").is_empty());
    }

    #[test]
    fn crlf_and_indent_are_accepted() {
        let d = parse("name = \"a\"\r\n  [x]\r\n\ty = 1\r\n");
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.tables[0].entries[0].key, "y");
    }

    #[test]
    fn trailing_garbage_is_flagged() {
        assert!(errs("a = 1 2")[0].contains("after the value"));
        assert!(errs("[x] y")[0].contains("after the value"));
        assert!(errs("a = [1] 2")[0].contains("after the value"));
    }
}
