//! Find and replace engine shared by the find bar and find-in-files.

use std::ops::Range;

use regex::{Regex, RegexBuilder};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub wrap: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex: false,
            wrap: true,
        }
    }
}

/// Compiled finder that only rebuilds its regex when the query changes.
pub struct Finder {
    query: String,
    options: SearchOptions,
    compiled: Option<Result<Regex, String>>,
}

impl Default for Finder {
    fn default() -> Self {
        Self::new()
    }
}

impl Finder {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            options: SearchOptions::default(),
            compiled: None,
        }
    }

    pub fn error(&self) -> Option<&str> {
        match &self.compiled {
            Some(Err(error)) => Some(error),
            _ => None,
        }
    }

    fn ensure(&mut self, query: &str, options: &SearchOptions) {
        if self.query == query && &self.options == options && self.compiled.is_some() {
            return;
        }
        self.query = query.to_owned();
        self.options = options.clone();
        self.compiled = Some(compile(query, options));
    }

    pub fn matches(
        &mut self,
        text: &str,
        query: &str,
        options: &SearchOptions,
    ) -> Vec<Range<usize>> {
        self.ensure(query, options);
        if query.is_empty() {
            return Vec::new();
        }
        match &self.compiled {
            Some(Ok(regex)) => regex
                .find_iter(text)
                .filter(|m| m.end() > m.start())
                .map(|m| m.start()..m.end())
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn replace_all(
        &mut self,
        text: &str,
        query: &str,
        options: &SearchOptions,
        replacement: &str,
    ) -> Result<String, String> {
        self.ensure(query, options);
        match &self.compiled {
            Some(Ok(regex)) => {
                if options.regex {
                    Ok(regex.replace_all(text, replacement).into_owned())
                } else {
                    Ok(regex
                        .replace_all(text, regex::NoExpand(replacement))
                        .into_owned())
                }
            }
            Some(Err(error)) => Err(error.clone()),
            None => Ok(text.to_owned()),
        }
    }

    pub fn replace_match(
        &mut self,
        matched_slice: &str,
        query: &str,
        options: &SearchOptions,
        replacement: &str,
    ) -> Result<String, String> {
        self.ensure(query, options);
        match &self.compiled {
            Some(Ok(regex)) if options.regex => Ok(regex.replace(matched_slice, replacement).into_owned()),
            Some(Err(error)) => Err(error.clone()),
            _ => Ok(replacement.to_owned()),
        }
    }
}

fn compile(query: &str, options: &SearchOptions) -> Result<Regex, String> {
    let pattern = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    let pattern = if options.whole_word {
        format!(r"\b(?:{pattern})\b")
    } else {
        pattern
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|error| error.to_string())
}

/// Index of the next match after `from` (byte offset), wrapping when enabled.
pub fn next_match(
    matches: &[Range<usize>],
    from: usize,
    forward: bool,
    wrap: bool,
) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    if forward {
        if let Some(i) = matches.iter().position(|m| m.start >= from) {
            return Some(i);
        }
        if wrap {
            return Some(0);
        }
    } else {
        if let Some(i) = matches.iter().rposition(|m| m.end <= from) {
            return Some(i);
        }
        if wrap {
            return Some(matches.len() - 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    fn options(regex: bool, whole: bool, case: bool) -> SearchOptions {
        SearchOptions {
            case_sensitive: case,
            whole_word: whole,
            regex,
            wrap: true,
        }
    }

    #[test]
    fn literal_and_case_insensitive() {
        let mut f = Finder::new();
        let text = "Hello hello HELLO";
        let m = f.matches(text, "hello", &options(false, false, false));
        assert_eq!(m, vec![0..5, 6..11, 12..17]);
        let m = f.matches(text, "hello", &options(false, false, true));
        assert_eq!(m, vec![6..11]);
    }

    #[test]
    fn whole_word() {
        let mut f = Finder::new();
        let text = "cat category cat.";
        let m = f.matches(text, "cat", &options(false, true, false));
        assert_eq!(m, vec![0..3, 13..16]);
    }

    #[test]
    fn regex_groups_and_replacement() {
        let mut f = Finder::new();
        let text = "a1 b2 c3";
        let opts = options(true, false, true);
        let m = f.matches(text, r"([a-z])(\d)", &opts);
        assert_eq!(m, vec![0..2, 3..5, 6..8]);
        let replaced = f.replace_all(text, r"([a-z])(\d)", &opts, "$2$1").unwrap();
        assert_eq!(replaced, "1a 2b 3c");
    }

    #[test]
    fn literal_replacement_keeps_dollar_signs() {
        let mut f = Finder::new();
        let opts = options(false, false, true);
        let replaced = f.replace_all("price", "price", &opts, "$5").unwrap();
        assert_eq!(replaced, "$5");
    }

    #[test]
    fn invalid_regex_reports_error() {
        let mut f = Finder::new();
        let opts = options(true, false, true);
        assert!(f.matches("x", "(", &opts).is_empty());
        assert!(f.error().is_some());
    }

    #[test]
    fn navigation_wraps() {
        let matches: Vec<Range<usize>> = vec![0..2, 10..12, 20..22];
        assert_eq!(next_match(&matches, 3, true, true), Some(1));
        assert_eq!(next_match(&matches, 30, true, true), Some(0));
        assert_eq!(next_match(&matches, 3, false, true), Some(0));
        assert_eq!(next_match(&matches, 30, true, false), None);
    }
}
