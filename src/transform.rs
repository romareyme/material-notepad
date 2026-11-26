//! Text transformation utilities, document statistics, and line-level diffing.

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextStats {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
    pub chars_no_ws: usize,
    pub bytes: usize,
}

pub fn compute_stats(text: &str) -> TextStats {
    let lines = if text.is_empty() {
        1
    } else {
        text.bytes().filter(|&b| b == b'\n').count() + 1
    };
    let words = text.split_whitespace().count();
    let mut chars = 0usize;
    let mut chars_no_ws = 0usize;
    for ch in text.chars() {
        chars += 1;
        if !ch.is_whitespace() {
            chars_no_ws += 1;
        }
    }
    TextStats {
        lines,
        words,
        chars,
        chars_no_ws,
        bytes: text.len(),
    }
}

pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn sort_lines(text: &str, descending: bool) -> String {
    let had_trailing_nl = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut lines: Vec<&str> = body.split('\n').collect();
    lines.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    if descending {
        lines.reverse();
    }
    let mut out = lines.join("\n");
    if had_trailing_nl {
        out.push('\n');
    }
    out
}

pub fn remove_duplicate_lines(text: &str) -> String {
    let had_trailing_nl = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut seen = HashSet::new();
    let mut out_lines = Vec::new();
    for line in body.split('\n') {
        if seen.insert(line) {
            out_lines.push(line);
        }
    }
    let mut out = out_lines.join("\n");
    if had_trailing_nl {
        out.push('\n');
    }
    out
}

pub fn remove_empty_lines(text: &str) -> String {
    let had_trailing_nl = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let out_lines: Vec<&str> = body
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut out = out_lines.join("\n");
    if had_trailing_nl && !out.is_empty() {
        out.push('\n');
    }
    out
}

pub fn trim_trailing_whitespace(text: &str) -> String {
    let had_trailing_nl = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    let out_lines: Vec<&str> = body
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect();
    let mut out = out_lines.join("\n");
    if had_trailing_nl {
        out.push('\n');
    }
    out
}

pub fn to_title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut new_word = true;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            if new_word {
                for up in ch.to_uppercase() {
                    out.push(up);
                }
                new_word = false;
            } else {
                for low in ch.to_lowercase() {
                    out.push(low);
                }
            }
        } else {
            new_word = true;
            out.push(ch);
        }
    }
    out
}

pub fn toggle_snake_camel(text: &str) -> String {
    // Process identifier-like tokens individually so multi-word or multi-line
    // selections convert cleanly.
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        if tok.is_empty() {
            return;
        }
        if tok.contains('_') {
            out.push_str(&snake_to_camel(tok));
        } else {
            out.push_str(&camel_to_snake(tok));
        }
        tok.clear();
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            token.push(ch);
        } else {
            flush(&mut token, &mut out);
            out.push(ch);
        }
    }
    flush(&mut token, &mut out);
    out
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = false;
    for (idx, part) in s.split('_').filter(|p| !p.is_empty()).enumerate() {
        if idx == 0 {
            out.push_str(&part.to_lowercase());
            upper_next = true;
        } else {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                if upper_next {
                    for c in first.to_uppercase() {
                        out.push(c);
                    }
                } else {
                    for c in first.to_lowercase() {
                        out.push(c);
                    }
                }
                out.push_str(&chars.as_str().to_lowercase());
            }
        }
    }
    if out.is_empty() {
        s.to_owned()
    } else {
        out
    }
}

fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for low in ch.to_lowercase() {
                out.push(low);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn format_json(text: &str, indent_spaces: usize) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let indent = " ".repeat(indent_spaces.clamp(1, 8));
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(&value, &mut ser).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

pub fn minify_json(text: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Same,
    Removed,
    Added,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub left_line: Option<usize>,
    pub right_line: Option<usize>,
    pub text: String,
}

/// Compute a line-by-line diff using LCS (bounded for very large files to keep UI responsive).
pub fn line_diff(left: &str, right: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = left.lines().collect();
    let b: Vec<&str> = right.lines().collect();
    let n = a.len();
    let m = b.len();

    // Trim common prefix & suffix so large files with small edits diff instantaneously.
    let mut prefix = 0usize;
    while prefix < n && prefix < m && a[prefix] == b[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix + prefix < n
        && suffix + prefix < m
        && a[n - 1 - suffix] == b[m - 1 - suffix]
    {
        suffix += 1;
    }

    let mut result = Vec::with_capacity(n.max(m));
    for (idx, line) in a[..prefix].iter().enumerate() {
        result.push(DiffLine {
            kind: DiffKind::Same,
            left_line: Some(idx + 1),
            right_line: Some(idx + 1),
            text: (*line).to_owned(),
        });
    }

    let mid_a = &a[prefix..n - suffix];
    let mid_b = &b[prefix..m - suffix];
    let ma = mid_a.len();
    let mb = mid_b.len();

    if ma * mb <= 2_500_000 {
        let mut dp = vec![0u32; (ma + 1) * (mb + 1)];
        let stride = mb + 1;
        for i in (0..ma).rev() {
            for j in (0..mb).rev() {
                dp[i * stride + j] = if mid_a[i] == mid_b[j] {
                    dp[(i + 1) * stride + (j + 1)] + 1
                } else {
                    dp[(i + 1) * stride + j].max(dp[i * stride + (j + 1)])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < ma && j < mb {
            if mid_a[i] == mid_b[j] {
                result.push(DiffLine {
                    kind: DiffKind::Same,
                    left_line: Some(prefix + i + 1),
                    right_line: Some(prefix + j + 1),
                    text: mid_a[i].to_owned(),
                });
                i += 1;
                j += 1;
            } else if dp[(i + 1) * stride + j] >= dp[i * stride + (j + 1)] {
                result.push(DiffLine {
                    kind: DiffKind::Removed,
                    left_line: Some(prefix + i + 1),
                    right_line: None,
                    text: mid_a[i].to_owned(),
                });
                i += 1;
            } else {
                result.push(DiffLine {
                    kind: DiffKind::Added,
                    left_line: None,
                    right_line: Some(prefix + j + 1),
                    text: mid_b[j].to_owned(),
                });
                j += 1;
            }
        }
        while i < ma {
            result.push(DiffLine {
                kind: DiffKind::Removed,
                left_line: Some(prefix + i + 1),
                right_line: None,
                text: mid_a[i].to_owned(),
            });
            i += 1;
        }
        while j < mb {
            result.push(DiffLine {
                kind: DiffKind::Added,
                left_line: None,
                right_line: Some(prefix + j + 1),
                text: mid_b[j].to_owned(),
            });
            j += 1;
        }
    } else {
        for (i, line) in mid_a.iter().enumerate() {
            result.push(DiffLine {
                kind: DiffKind::Removed,
                left_line: Some(prefix + i + 1),
                right_line: None,
                text: (*line).to_owned(),
            });
        }
        for (j, line) in mid_b.iter().enumerate() {
            result.push(DiffLine {
                kind: DiffKind::Added,
                left_line: None,
                right_line: Some(prefix + j + 1),
                text: (*line).to_owned(),
            });
        }
    }

    for k in 0..suffix {
        let li = n - suffix + k;
        let ri = m - suffix + k;
        result.push(DiffLine {
            kind: DiffKind::Same,
            left_line: Some(li + 1),
            right_line: Some(ri + 1),
            text: a[li].to_owned(),
        });
    }
    result
}

/// Parse CSV/TSV text into rows of fields, respecting quoted fields and `""` escapes.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let delimiter = detect_csv_delimiter(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
        } else if ch == '"' && field.is_empty() {
            in_quotes = true;
        } else if ch == delimiter {
            row.push(std::mem::take(&mut field));
        } else if ch == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else if ch == '\r' {
            // Ignore CR (text is normally LF-normalized).
        } else {
            field.push(ch);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

pub fn detect_csv_delimiter(text: &str) -> char {
    let sample: String = text.lines().take(5).collect::<Vec<_>>().join("\n");
    let commas = sample.matches(',').count();
    let semis = sample.matches(';').count();
    let tabs = sample.matches('\t').count();
    if tabs > commas && tabs > semis {
        '\t'
    } else if semis > commas {
        ';'
    } else {
        ','
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_dedups_and_trims() {
        assert_eq!(sort_lines("b\na\nc\n", false), "a\nb\nc\n");
        assert_eq!(sort_lines("b\na\nc\n", true), "c\nb\na\n");
        assert_eq!(remove_duplicate_lines("a\nb\na\nc"), "a\nb\nc");
        assert_eq!(remove_empty_lines("a\n\n  \nb\n"), "a\nb\n");
        assert_eq!(trim_trailing_whitespace("a  \t\nb \n"), "a\nb\n");
    }

    #[test]
    fn case_and_json_transforms() {
        assert_eq!(to_title_case("hello WORLD_test"), "Hello World_Test");
        assert_eq!(toggle_snake_camel("hello_world"), "helloWorld");
        assert_eq!(toggle_snake_camel("helloWorld"), "hello_world");
        let pretty = format_json("{\"a\":1,\"b\":[2]}", 2).unwrap();
        assert!(pretty.contains('\n'));
        assert_eq!(minify_json(&pretty).unwrap(), "{\"a\":1,\"b\":[2]}");
    }

    #[test]
    fn diff_detects_changes() {
        let diff = line_diff("a\nb\nc", "a\nx\nc\nd");
        assert!(diff.iter().any(|l| l.kind == DiffKind::Removed && l.text == "b"));
        assert!(diff.iter().any(|l| l.kind == DiffKind::Added && l.text == "x"));
        assert!(diff.iter().any(|l| l.kind == DiffKind::Added && l.text == "d"));
    }

    #[test]
    fn parses_quoted_csv() {
        let rows = parse_csv("name,note\n\"Ada\",\"hi, \"\"world\"\"\"\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], vec!["Ada", "hi, \"world\""]);
    }
}
