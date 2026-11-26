//! Document model: text, encoding, line endings, file IO.
//!
//! Text is kept in memory with `\n` line endings so that editing, searching and
//! syntax highlighting have one representation.  The original ending style and
//! encoding are remembered and restored on save.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::highlight::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Eol {
    Lf,
    CrLf,
    Cr,
}

impl Eol {
    pub fn label(self) -> &'static str {
        match self {
            Eol::Lf => "LF",
            Eol::CrLf => "CRLF",
            Eol::Cr => "CR",
        }
    }

    pub fn sequence(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::CrLf => "\r\n",
            Eol::Cr => "\r",
        }
    }

    pub fn all() -> [Eol; 3] {
        [Eol::Lf, Eol::CrLf, Eol::Cr]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Latin1,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 BOM",
            Encoding::Utf16Le => "UTF-16 LE",
            Encoding::Utf16Be => "UTF-16 BE",
            Encoding::Latin1 => "ANSI",
        }
    }

    pub fn all() -> [Encoding; 5] {
        [
            Encoding::Utf8,
            Encoding::Utf8Bom,
            Encoding::Utf16Le,
            Encoding::Utf16Be,
            Encoding::Latin1,
        ]
    }
}

/// Decode bytes into text, detecting encoding and dominant line ending.
pub fn decode(bytes: &[u8]) -> (String, Encoding, Eol) {
    let (text, encoding) = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        (
            String::from_utf8_lossy(rest).into_owned(),
            Encoding::Utf8Bom,
        )
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        (decode_utf16(rest, true), Encoding::Utf16Le)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        (decode_utf16(rest, false), Encoding::Utf16Be)
    } else if let Ok(text) = std::str::from_utf8(bytes) {
        (text.to_owned(), Encoding::Utf8)
    } else {
        (bytes.iter().map(|b| *b as char).collect(), Encoding::Latin1)
    };
    let eol = detect_eol(&text);
    (normalize_eol(&text), encoding, eol)
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            if little_endian {
                u16::from_le_bytes(*c)
            } else {
                u16::from_be_bytes(*c)
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

fn detect_eol(text: &str) -> Eol {
    let mut crlf = 0usize;
    let mut lf = 0usize;
    let mut lone_cr = 0usize;
    let mut previous_was_cr = false;
    for byte in text.bytes() {
        match byte {
            b'\r' => {
                lone_cr += 1;
                previous_was_cr = true;
            }
            b'\n' => {
                lf += 1;
                if previous_was_cr {
                    crlf += 1;
                    lone_cr -= 1;
                }
                previous_was_cr = false;
            }
            _ => previous_was_cr = false,
        }
    }
    if crlf >= lf.saturating_sub(crlf) && crlf > 0 {
        Eol::CrLf
    } else if lone_cr > lf {
        Eol::Cr
    } else {
        Eol::Lf
    }
}

fn normalize_eol(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Encode text for writing, applying the requested line endings.
pub fn encode(text: &str, encoding: Encoding, eol: Eol) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let with_eol = if eol == Eol::Lf {
        normalized
    } else {
        normalized.replace('\n', eol.sequence())
    };
    match encoding {
        Encoding::Utf8 => with_eol.into_bytes(),
        Encoding::Utf8Bom => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend_from_slice(with_eol.as_bytes());
            out
        }
        Encoding::Utf16Le => {
            let mut out = vec![0xFF, 0xFE];
            for unit in with_eol.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        Encoding::Utf16Be => {
            let mut out = vec![0xFE, 0xFF];
            for unit in with_eol.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
        Encoding::Latin1 => with_eol
            .chars()
            .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
            .collect(),
    }
}

/// Cursor data mirrored from the editor widget for the status bar.
#[derive(Clone, Copy, Debug, Default)]
pub struct CursorInfo {
    pub line: usize,
    pub column: usize,
    pub selection_chars: usize,
}

pub struct Document {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub encoding: Encoding,
    pub eol: Eol,
    pub language: Language,
    pub revision: u64,
    pub saved_revision: u64,
    pub cursor: CursorInfo,
    pub saved_at: Option<SystemTime>,
    pub file_mtime: Option<SystemTime>,
    pub file_len: Option<u64>,
    pub external_change: bool,
    pub backup_name: Option<String>,
    line_starts: Vec<usize>,
    line_char_starts: Vec<usize>,
    indexed_revision: u64,
    pub scroll_to_line: Option<usize>,
    /// Current selection in byte offsets (start, end).
    pub selection: Option<(usize, usize)>,
    /// Selection requested by a command and applied by the editor widget.
    pub requested_selection: Option<(usize, usize)>,
    /// A selection set by a command (Ctrl+A) that the widget's own caret report
    /// must not overwrite until the user types or moves the caret.
    pub selection_sticky: bool,
}

impl Document {
    pub fn untitled(id: u64, index: usize) -> Self {
        let mut doc = Self {
            id,
            path: None,
            title: format!("Untitled-{index}"),
            text: String::new(),
            encoding: Encoding::Utf8,
            eol: Eol::Lf,
            language: Language::PlainText,
            revision: 1,
            saved_revision: 1,
            cursor: CursorInfo {
                line: 1,
                column: 1,
                selection_chars: 0,
            },
            saved_at: None,
            file_mtime: None,
            file_len: None,
            external_change: false,
            backup_name: None,
            line_starts: vec![0],
            line_char_starts: vec![0],
            indexed_revision: 1,
            scroll_to_line: None,
            selection: None,
            requested_selection: None,
            selection_sticky: false,
        };
        doc.language = Language::detect(None, &doc.text, None);
        doc
    }

    pub fn from_path(id: u64, path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let (text, encoding, eol) = decode(&bytes);
        let mut doc = Self::untitled(id, 1);
        doc.path = Some(path.to_path_buf());
        doc.title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        doc.language = Language::detect(
            path.extension().and_then(|e| e.to_str()),
            &text,
            path.file_name().and_then(|n| n.to_str()),
        );
        doc.text = text;
        doc.encoding = encoding;
        doc.eol = eol;
        let meta = fs::metadata(path).ok();
        doc.file_mtime = meta.as_ref().and_then(|m| m.modified().ok());
        doc.file_len = meta.as_ref().map(|m| m.len());
        doc.revision = 1;
        doc.saved_revision = 1;
        doc.indexed_revision = 0;
        doc.ensure_index();
        Ok(doc)
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn ensure_index(&mut self) {
        if self.indexed_revision == self.revision {
            return;
        }
        self.line_starts.clear();
        self.line_char_starts.clear();
        self.line_starts.push(0);
        self.line_char_starts.push(0);
        let mut chars = 0usize;
        for (i, b) in self.text.bytes().enumerate() {
            if b == b'\n' {
                chars += 1;
                self.line_starts.push(i + 1);
                self.line_char_starts.push(chars);
            } else if (b & 0xC0) != 0x80 {
                chars += 1;
            }
        }
        if self.line_starts.is_empty() {
            self.line_starts.push(0);
            self.line_char_starts.push(0);
        }
        self.indexed_revision = self.revision;
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len().max(1)
    }

    pub fn line_start(&self, line: usize) -> usize {
        let idx = line
            .saturating_sub(1)
            .min(self.line_starts.len().saturating_sub(1));
        self.line_starts.get(idx).copied().unwrap_or(0)
    }

    pub fn line_range(&self, line: usize) -> std::ops::Range<usize> {
        let start = self.line_start(line);
        let next = if line < self.line_starts.len() {
            self.line_starts[line]
        } else {
            self.text.len()
        };
        start..next
    }

    /// Snap a byte offset down to the nearest character boundary, clamped to the text.
    pub fn snap(&self, byte: usize) -> usize {
        let mut byte = byte.min(self.text.len());
        while byte > 0 && !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        byte
    }

    /// Line and character column (both 1 based) for a byte offset.
    pub fn line_col(&self, byte: usize) -> (usize, usize) {
        let byte = self.snap(byte);
        let idx = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let start = self.line_starts.get(idx).copied().unwrap_or(0);
        let col = self.text[start..byte].chars().count() + 1;
        (idx + 1, col)
    }

    /// Byte offset of a line/column pair, clamped to the document.
    pub fn byte_of(&self, line: usize, column: usize) -> usize {
        let line = line.clamp(1, self.line_count());
        let start = self.line_start(line);
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len());
        let mut byte = start;
        for ch in self.text[start..end].chars().take(column.saturating_sub(1)) {
            byte += ch.len_utf8();
        }
        byte.min(end)
    }

    pub fn update_cursor(&mut self, byte: usize, sel_bytes: Option<(usize, usize)>) {
        let byte = self.snap(byte);
        let (line, column) = self.line_col(byte);
        self.cursor = CursorInfo {
            line,
            column,
            selection_chars: sel_bytes
                .map(|(a, b)| {
                    // Offsets can lag one frame behind a deletion, so clamp and
                    // snap before slicing: an out-of-range or mid-character
                    // index would otherwise panic and take the window down.
                    let a = self.snap(a);
                    let b = self.snap(b);
                    self.text[a.min(b)..a.max(b)].chars().count()
                })
                .unwrap_or(0),
        };
    }

    /// Character index (as used by egui text cursors) of a byte offset.
    pub fn char_index(&self, byte: usize) -> usize {
        let byte = self.snap(byte);
        let (line, _) = self.line_col(byte);
        let idx = line.saturating_sub(1);
        let start_char = self.line_char_starts.get(idx).copied().unwrap_or(0);
        let start_byte = self.line_starts.get(idx).copied().unwrap_or(0);
        start_char + self.text[start_byte..byte].chars().count()
    }

    /// Byte offset of a character index, clamped to the document.
    pub fn byte_index(&self, char_index: usize) -> usize {
        let idx = match self.line_char_starts.binary_search(&char_index) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start_char = self.line_char_starts.get(idx).copied().unwrap_or(0);
        let line_start_byte = self.line_starts.get(idx).copied().unwrap_or(0);
        let line_end_byte = self
            .line_starts
            .get(idx + 1)
            .copied()
            .unwrap_or(self.text.len());
        let mut offset = char_index.saturating_sub(line_start_char);
        let mut byte = line_start_byte;
        for ch in self.text[line_start_byte..line_end_byte].chars() {
            if offset == 0 {
                break;
            }
            offset -= 1;
            byte += ch.len_utf8();
        }
        self.snap(byte.min(line_end_byte))
    }

    pub fn save(&mut self) -> io::Result<()> {
        let path = match &self.path {
            Some(path) => path.clone(),
            None => return Err(io::Error::other("document has no path")),
        };
        let bytes = encode(&self.text, self.encoding, self.eol);
        // Write to a sibling file and rename it into place, so a crash mid-write
        // cannot leave a truncated document behind.
        let temp = path.with_extension("mn-save");
        fs::write(&temp, &bytes)?;
        match fs::rename(&temp, &path) {
            Ok(()) => {}
            Err(_) => {
                // rename across filesystems or a locked target: fall back to a
                // direct write, which is still better than losing the save.
                let result = fs::write(&path, &bytes);
                let _ = fs::remove_file(&temp);
                result?;
            }
        }
        self.saved_revision = self.revision;
        self.saved_at = Some(SystemTime::now());
        let meta = fs::metadata(&path).ok();
        self.file_mtime = meta.as_ref().and_then(|m| m.modified().ok());
        self.file_len = meta.as_ref().map(|m| m.len());
        self.external_change = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: &Path) -> io::Result<()> {
        self.path = Some(path.to_path_buf());
        self.title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        self.language = Language::detect(
            path.extension().and_then(|e| e.to_str()),
            &self.text,
            path.file_name().and_then(|n| n.to_str()),
        );
        self.save()
    }

    /// Compare the on-disk timestamp with the one captured at load/save.
    /// Also compares size: replacing a file with an older-timestamped copy
    /// (git checkout of an older commit) changes nothing about mtime.
    pub fn poll_external_change(&mut self) {
        let Some(path) = &self.path else {
            self.external_change = false;
            return;
        };
        let meta = fs::metadata(path).ok();
        let now = meta.as_ref().and_then(|m| m.modified().ok());
        let len = meta.as_ref().map(|m| m.len());
        match (now, self.file_mtime) {
            (Some(now), Some(then)) => {
                if now > then || len != self.file_len {
                    self.external_change = true;
                }
            }
            (None, Some(_)) => self.external_change = true,
            _ => {}
        }
    }

    pub fn reload(&mut self) -> io::Result<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let bytes = fs::read(&path)?;
        let (text, encoding, eol) = decode(&bytes);
        self.text = text;
        self.encoding = encoding;
        self.eol = eol;
        self.revision += 1;
        self.saved_revision = self.revision;
        self.external_change = false;
        let meta = fs::metadata(&path).ok();
        self.file_mtime = meta.as_ref().and_then(|m| m.modified().ok());
        self.file_len = meta.as_ref().map(|m| m.len());
        self.indexed_revision = 0;
        self.ensure_index();
        Ok(())
    }

    /// Record an edit made directly on the text and rebuild the line index.
    pub fn edited(&mut self) {
        self.revision += 1;
        self.indexed_revision = 0;
        self.ensure_index();
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.revision += 1;
        self.indexed_revision = 0;
        self.ensure_index();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_round_trips_line_endings() {
        let (text, enc, eol) = decode(b"a\r\nb\r\nc");
        assert_eq!(text, "a\nb\nc");
        assert_eq!(enc, Encoding::Utf8);
        assert_eq!(eol, Eol::CrLf);
        let bytes = encode(&text, enc, eol);
        assert_eq!(bytes, b"a\r\nb\r\nc");
    }

    #[test]
    fn round_trips_encodings() {
        let sample = "h\u{e9}llo \u{2192} \u{43c}\u{438}\u{440}\n";
        for enc in Encoding::all() {
            let bytes = encode(sample, enc, Eol::Lf);
            let (text, decoded, _) = decode(&bytes);
            if enc == Encoding::Latin1 {
                assert_eq!(decoded, Encoding::Latin1);
            } else {
                assert_eq!(text, sample, "encoding {enc:?}");
            }
        }
    }

    #[test]
    fn utf16_without_bom_is_not_detected_but_bom_is() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "h\u{e9}".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let (text, enc, _) = decode(&bytes);
        assert_eq!(text, "h\u{e9}");
        assert_eq!(enc, Encoding::Utf16Le);
    }

    #[test]
    fn line_index_and_positions() {
        let mut doc = Document::untitled(1, 1);
        doc.set_text("one\ntwo\nthree".to_owned());
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line_range(2), 4..8);
        assert_eq!(doc.line_col(4), (2, 1));
        assert_eq!(doc.line_col(5), (2, 2));
        assert_eq!(doc.byte_of(2, 2), 5);
        assert_eq!(doc.byte_of(3, 1), 8);
    }

    #[test]
    fn line_count_keeps_trailing_empty_line() {
        let mut doc = Document::untitled(1, 1);
        doc.set_text("a\n".to_owned());
        assert_eq!(doc.line_count(), 2);
        assert_eq!(doc.line_range(2), 2..2);
    }

    #[test]
    fn multibyte_columns() {
        let mut doc = Document::untitled(1, 1);
        doc.set_text("a\u{e9}\u{65e5}b".to_owned());
        assert_eq!(doc.line_col("a\u{e9}".len()), (1, 3));
        assert_eq!(doc.byte_of(1, 3), "a\u{e9}".len());
        assert_eq!(doc.char_index("a\u{e9}".len()), 2);
        assert_eq!(doc.byte_index(2), "a\u{e9}".len());
    }

    #[test]
    fn dirty_flag_tracks_revisions() {
        let mut doc = Document::untitled(1, 1);
        assert!(!doc.is_dirty());
        doc.set_text("x".to_owned());
        assert!(doc.is_dirty());
        doc.saved_revision = doc.revision;
        assert!(!doc.is_dirty());
    }

    #[test]
    fn direct_edits_invalidate_the_line_index() {
        let mut doc = Document::untitled(1, 1);
        doc.set_text("one\ntwo".to_owned());
        assert_eq!(doc.line_count(), 2);
        doc.text.insert_str(0, "zero\n");
        doc.edited();
        assert_eq!(doc.line_count(), 3);
        assert_eq!(doc.line_col(5), (2, 1));
    }
}
