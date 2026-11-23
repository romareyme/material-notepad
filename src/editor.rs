//! The editor surface: syntax-coloured text, gutter, overlays and commands.
//!
//! Text input, selection, IME and clipboard handling come from egui's
//! `TextEdit`; this module supplies the Material look (gutter, current line,
//! find highlights) and the editor commands Notepad Next exposes.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, Margin, Modifiers, Pos2, Rect, ScrollArea, Sense,
    Stroke, TextEdit, TextFormat, Ui, Vec2,
};

use crate::document::{CursorInfo, Document};
use crate::highlight::{highlight, Language, Span};
use crate::theme::Theme;

/// Empty space between the gutter separator and the first character.
const GUTTER_TEXT_GAP: f32 = 8.0;

/// How much text gets syntax highlighting before falling back to plain text.
const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct EditorPrefs {
    pub tab_width: usize,
    pub use_spaces: bool,
    pub word_wrap: bool,
    pub line_numbers: bool,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            tab_width: 4,
            use_spaces: true,
            word_wrap: false,
            line_numbers: true,
        }
    }
}

#[derive(Default)]
pub struct EditorEvent {
    pub changed: bool,
    pub cursor: CursorInfo,
}

struct CacheEntry {
    revision: u64,
    language: Language,
    dark: bool,
    spans: Arc<Vec<Span>>,
}

#[derive(Default)]
pub struct HighlightCache {
    entries: HashMap<u64, CacheEntry>,
    order: Vec<u64>,
}

impl HighlightCache {
    fn spans(&mut self, doc: &Document, theme: &Theme) -> Arc<Vec<Span>> {
        if let Some(entry) = self.entries.get(&doc.id) {
            if entry.revision == doc.revision
                && entry.language == doc.language
                && entry.dark == theme.dark
            {
                return entry.spans.clone();
            }
        }
        let spans = if doc.text.len() > MAX_HIGHLIGHT_BYTES {
            Arc::new(Vec::new())
        } else {
            Arc::new(highlight(&doc.text, doc.language, &theme.syntax()))
        };
        self.entries.insert(
            doc.id,
            CacheEntry {
                revision: doc.revision,
                language: doc.language,
                dark: theme.dark,
                spans: spans.clone(),
            },
        );
        self.order.retain(|id| *id != doc.id);
        self.order.push(doc.id);
        while self.order.len() > 8 {
            let oldest = self.order.remove(0);
            self.entries.remove(&oldest);
        }
        spans
    }

    pub fn forget(&mut self, id: u64) {
        self.entries.remove(&id);
        self.order.retain(|entry| *entry != id);
    }
}

fn layout_job(text: &str, spans: &[Span], theme: &Theme, wrap_width: f32) -> egui::text::LayoutJob {
    let colors = theme.syntax();
    let base = TextFormat {
        font_id: theme.editor_font(),
        color: colors.text,
        line_height: Some(theme.editor_line_height),
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let mut pos = 0usize;
    for span in spans {
        if span.range.start > pos {
            job.append(&text[pos..span.range.start], 0.0, base.clone());
        }
        let format = TextFormat {
            color: span.kind.color(&colors),
            ..base.clone()
        };
        job.append(&text[span.range.clone()], 0.0, format);
        pos = span.range.end;
    }
    if pos < text.len() {
        job.append(&text[pos..], 0.0, base);
    }
    job
}

fn apply_requested_selection(ui: &Ui, id: Id, doc: &mut Document) -> bool {
    let Some((start, end)) = doc.requested_selection.take() else {
        return false;
    };
    let a = doc.char_index(start.min(end));
    let b = doc.char_index(start.max(end));
    let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
    state.store(ui.ctx(), id);
    ui.memory_mut(|m| m.request_focus(id));
    doc.selection = Some((start.min(end), start.max(end)));
    true
}

use eframe::egui::text_edit::TextEditState;

/// Draw the editor for one document and return what changed.
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    doc: &mut Document,
    prefs: &EditorPrefs,
    matches: &[Range<usize>],
    current_match: Option<usize>,
    cache: &mut HighlightCache,
) -> EditorEvent {
    doc.ensure_index();
    let id = Id::new(("editor", doc.id));
    let editor_font = theme.editor_font();
    let digit_width = ui
        .fonts(|f| f.layout_no_wrap("0".to_owned(), editor_font.clone(), Color32::WHITE))
        .size()
        .x;
    let gutter = if prefs.line_numbers {
        (digit_width * doc.line_count().to_string().len() as f32 + 12.0).max(30.0)
    } else {
        0.0
    };
    let apply_selection = |ui: &Ui, doc: &mut Document| apply_requested_selection(ui, id, doc);
    let mut selection_requested = apply_selection(ui, doc);

    // Editor shortcuts apply to the active document whether or not the widget
    // currently owns keyboard focus (a menu or dialog can take it).
    handle_keys(ui, doc, prefs);
    selection_requested |= apply_selection(ui, doc);

    let spans = cache.spans(doc, theme);
    let background = theme.scheme.surface_container_lowest;
    let gutter_bg = theme.scheme.surface_container_low;
    let gutter_fg = theme.scheme.outline;
    let gutter_active = theme.scheme.primary;
    let separator = theme.scheme.outline_variant;
    let line_highlight = Color32::from_rgba_unmultiplied(
        theme.scheme.primary.r(),
        theme.scheme.primary.g(),
        theme.scheme.primary.b(),
        20,
    );
    let match_fill = Color32::from_rgba_unmultiplied(
        theme.scheme.primary.r(),
        theme.scheme.primary.g(),
        theme.scheme.primary.b(),
        48,
    );
    let active_match_fill = Color32::from_rgba_unmultiplied(
        theme.scheme.tertiary.r(),
        theme.scheme.tertiary.g(),
        theme.scheme.tertiary.b(),
        90,
    );
    let word_wrap = prefs.word_wrap;

    let mut event = EditorEvent::default();
    let mut scroll_to_line = doc.scroll_to_line.take();
    let viewport_rows = ((ui.available_height() / theme.editor_line_height).ceil() as usize).max(6);

    let scroll = ScrollArea::both()
        .id_source(("editor-scroll", doc.id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // The scroll area's clip rect can be the whole screen; bound the
            // backing fill to the editor area itself.
            ui.painter()
                .rect_filled(ui.max_rect().intersect(ui.clip_rect()), 0.0, background);
            let width = (ui.available_width() - gutter - GUTTER_TEXT_GAP).max(120.0);
            let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
                let wrap = if word_wrap { wrap_width } else { f32::INFINITY };
                let job = layout_job(text, &spans, theme, wrap);
                ui.fonts(|fonts| fonts.layout_job(job))
            };
            let output = TextEdit::multiline(&mut doc.text)
                .id(id)
                .frame(false)
                .code_editor()
                .desired_rows(viewport_rows)
                .hint_text(
                    egui::RichText::new("Start typing, or open a file with Ctrl+O")
                        .color(theme.scheme.outline)
                        .font(theme.editor_font()),
                )
                .desired_width(width)
                .margin(Margin {
                    // Keep the text clear of the gutter divider.
                    left: gutter + GUTTER_TEXT_GAP,
                    right: 12.0,
                    top: 8.0,
                    bottom: 24.0,
                })
                .layouter(&mut layouter)
                .show(ui);

            let remaining = ui.available_height();
            if remaining > 1.0 {
                let rect = Rect::from_min_size(
                    ui.cursor().min,
                    Vec2::new(ui.available_width().max(10.0), remaining),
                );
                let response = ui.allocate_rect(rect, Sense::click());
                if response.clicked() {
                    doc.requested_selection = Some((doc.text.len(), doc.text.len()));
                    ui.memory_mut(|m| m.request_focus(id));
                }
                ui.painter().rect_filled(
                    rect.intersect(ui.max_rect()).intersect(ui.clip_rect()),
                    0.0,
                    background,
                );
            }

            if output.response.changed() {
                doc.edited();
                event.changed = true;
            }

            if let Some(range) = output.cursor_range {
                let primary = range.primary.ccursor.index;
                let secondary = range.secondary.ccursor.index;
                let a = doc.byte_index(primary);
                let b = doc.byte_index(secondary);
                // A selection this frame set by a command wins over the widget's
                // report for one frame, which still shows the previous caret.
                // A sticky selection from Ctrl+A keeps winning until the user
                // types or moves, because the widget reports its stale caret.
                if !selection_requested && !doc.selection_sticky {
                    doc.selection = Some((a.min(b), a.max(b)));
                }
                doc.update_cursor(a, Some((a, b)));

                // Backspace/Delete over a selection deletes it here, against the
                // offsets the widget reported (or the ones a command just set).
                // Leaving it to the TextEdit could take the window down with a
                // stale caret index.
                let deleting = ui.input(|i| {
                    i.key_pressed(Key::Backspace)
                        || i.key_pressed(Key::Delete)
                        || i.events.iter().any(|event| {
                            matches!(
                                event,
                                egui::Event::Key {
                                    key: Key::Backspace | Key::Delete,
                                    pressed: true,
                                    ..
                                }
                            )
                        })
                });
                let selected = doc.selection.filter(|(start, end)| end > start);
                if deleting {
                    if let Some((start, end)) = selected {
                        // The widget may already have deleted the text this
                        // frame, leaving these offsets past the end. Clamping
                        // them is what keeps the window alive.
                        let start = doc.snap(start);
                        let end = doc.snap(end).max(start);
                        if end > start {
                            doc.text.replace_range(start..end, "");
                            doc.edited();
                            event.changed = true;
                            doc.selection = Some((start, start));
                            doc.requested_selection = Some((start, start));
                            doc.selection_sticky = false;
                            doc.update_cursor(start, Some((start, start)));
                            apply_selection(ui, doc);
                        }
                    }
                }
                // Typing or moving the caret clears a command-made selection.
                // (response.changed() also fires when the state is stored, so it
                // is not a usable signal here.)
                let typed_or_moved = ui.input(|i| {
                    i.events.iter().any(|event| match event {
                        egui::Event::Text(text) => !text.is_empty(),
                        egui::Event::Key {
                            key, pressed: true, ..
                        } => matches!(
                            key,
                            Key::ArrowLeft
                                | Key::ArrowRight
                                | Key::ArrowUp
                                | Key::ArrowDown
                                | Key::Home
                                | Key::End
                                | Key::PageUp
                                | Key::PageDown
                        ),
                        _ => false,
                    })
                });
                if typed_or_moved {
                    doc.selection_sticky = false;
                }
            }
            event.cursor = doc.cursor;

            // Gutter.
            if prefs.line_numbers && ui.is_rect_visible(output.response.rect) {
                paint_gutter(
                    ui,
                    &output,
                    doc,
                    gutter,
                    theme.editor_font_size,
                    gutter_bg,
                    gutter_fg,
                    gutter_active,
                    separator,
                );
            }

            // Current line + find matches are painted after the text with a low
            // alpha so glyphs stay readable, Material style.
            let painter = ui.painter_at(output.text_clip_rect);
            let origin = output.galley_pos;
            if let Some(rect) = line_rect(&output.galley, origin, doc.cursor.line) {
                painter.rect_filled(rect, 0.0, line_highlight);
            }
            for (index, range) in matches.iter().enumerate() {
                if let Some(rect) = range_rect(doc, &output.galley, origin, range) {
                    if Some(index) == current_match {
                        painter.rect_filled(rect.expand(1.0), 2.0, active_match_fill);
                        painter.rect_stroke(
                            rect.expand(1.0),
                            2.0,
                            Stroke::new(1.0_f32, theme.scheme.tertiary),
                        );
                    } else {
                        painter.rect_filled(rect, 2.0, match_fill);
                    }
                }
            }

            if let Some(line) = scroll_to_line.take() {
                if let Some(rect) = line_rect(&output.galley, origin, line) {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
            }
        });

    let _ = scroll;
    event
}

#[allow(clippy::too_many_arguments)]
fn paint_gutter(
    ui: &Ui,
    output: &egui::text_edit::TextEditOutput,
    doc: &Document,
    gutter: f32,
    font_size: f32,
    bg: Color32,
    fg: Color32,
    active_fg: Color32,
    separator: Color32,
) {
    let rect = output.response.rect;
    let origin = output.galley_pos;
    let bottom = ui.max_rect().bottom().max(rect.bottom());
    let painter = ui.painter_at(Rect::from_min_max(rect.min, Pos2::new(rect.max.x, bottom)));
    // The number column is exactly `gutter` wide; the text starts `GUTTER_TEXT_GAP`
    // further right, so the divider sits between them instead of under the letters.
    let gutter_right = rect.min.x + gutter;
    let gutter_rect = Rect::from_min_max(rect.min, Pos2::new(gutter_right, bottom));
    painter.rect_filled(gutter_rect, 0.0, bg);
    painter.line_segment(
        [
            Pos2::new(gutter_right - 0.5, rect.top()),
            Pos2::new(gutter_right - 0.5, rect.bottom()),
        ],
        Stroke::new(1.0_f32, separator),
    );
    let font = FontId::new(font_size.max(9.0), egui::FontFamily::Monospace);
    let mut line_no = 1usize;
    let mut first_row = true;
    for row in &output.galley.rows {
        if first_row {
            let y = origin.y + row.rect.center().y;
            let color = if line_no == doc.cursor.line {
                active_fg
            } else {
                fg
            };
            painter.text(
                Pos2::new(gutter_right - 6.0, y),
                Align2::RIGHT_CENTER,
                line_no.to_string(),
                font.clone(),
                color,
            );
        }
        first_row = row.ends_with_newline;
        if row.ends_with_newline {
            line_no += 1;
        }
    }
    let _ = doc.line_count();
}

fn line_rect(galley: &egui::Galley, origin: Pos2, line: usize) -> Option<Rect> {
    let mut current = 1usize;
    for row in &galley.rows {
        if current == line {
            let mut rect = row.rect;
            rect = rect.translate(origin.to_vec2());
            rect.min.x = origin.x;
            rect.max.x = rect.max.x.max(origin.x + 1.0);
            return Some(rect);
        }
        if row.ends_with_newline {
            current += 1;
        }
    }
    None
}

fn range_rect(
    doc: &Document,
    galley: &egui::Galley,
    origin: Pos2,
    range: &Range<usize>,
) -> Option<Rect> {
    let start = doc.char_index(range.start);
    let end = doc.char_index(range.end);
    if start == end {
        return None;
    }
    let a = galley.pos_from_ccursor(CCursor::new(start));
    let b = galley.pos_from_ccursor(CCursor::new(end));
    let mut rect = Rect::from_min_max(a.min, b.max);
    rect = rect.translate(origin.to_vec2());
    if rect.width() < 2.0 {
        rect.max.x = rect.min.x + 2.0;
    }
    Some(rect)
}

fn handle_keys(ui: &mut Ui, doc: &mut Document, prefs: &EditorPrefs) {
    // egui 0.27 has no "select all" for TextEdit, so Ctrl+A is handled here.
    // The modifiers come from the event itself: input.modifiers can lag a frame.
    let select_all = ui.input(|i| {
        i.events.iter().any(|event| match event {
            egui::Event::Key {
                key: Key::A,
                pressed: true,
                modifiers,
                ..
            } => modifiers.ctrl || modifiers.command,
            _ => false,
        }) || ((i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::A))
    });
    if select_all {
        let len = doc.text.len();
        doc.selection = Some((0, len));
        doc.requested_selection = Some((0, len));
        doc.selection_sticky = true;
        doc.update_cursor(len, Some((0, len)));
    }

    let shift_tab = ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab));
    let tab = if shift_tab {
        false
    } else {
        ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
    };
    if shift_tab || tab {
        let selection = doc.selection.map(|(a, b)| (a.min(b), a.max(b)));
        let start_line = selection
            .map(|(a, _)| doc.line_col(a).0)
            .unwrap_or(doc.cursor.line);
        let end_line = selection
            .map(|(_, b)| doc.line_col(b).0)
            .unwrap_or(doc.cursor.line);
        if end_line > start_line {
            indent_lines(doc, start_line, end_line, !tab, prefs);
        } else if tab {
            let unit = if prefs.use_spaces {
                " ".repeat(prefs.tab_width)
            } else {
                "\t".to_owned()
            };
            replace_selection(doc, &unit);
        } else {
            outdent_line(doc, start_line, prefs);
        }
    }

    // Insert the newline into the buffer instead of relying on an injected text
    // event, which egui's TextEdit does not turn into a line break.
    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
        insert_newline(doc);
    }
}

/// Byte offset of the caret (or the start of the selection).
fn caret_byte(doc: &Document) -> usize {
    doc.selection
        .map(|(a, b)| a.min(b))
        .unwrap_or_else(|| doc.byte_of(doc.cursor.line, doc.cursor.column))
        .min(doc.text.len())
}

/// Replace the selection (or insert at the caret) and place the caret after it.
fn replace_selection(doc: &mut Document, text: &str) -> usize {
    let (start, end) = match doc.selection {
        Some((a, b)) => (a.min(b).min(doc.text.len()), a.max(b).min(doc.text.len())),
        None => {
            let caret = caret_byte(doc);
            (caret, caret)
        }
    };
    doc.text.replace_range(start..end, text);
    doc.edited();
    let caret = start + text.len();
    doc.requested_selection = Some((caret, caret));
    doc.selection = Some((caret, caret));
    caret
}

/// Insert a line break, keeping the indentation of the current line.
fn insert_newline(doc: &mut Document) {
    let byte = caret_byte(doc);
    let line = doc.line_col(byte).0;
    let indent: String = doc.text[doc.line_range(line)]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let mut text = String::from("\n");
    text.push_str(&indent);
    replace_selection(doc, &text);
}

fn indent_lines(
    doc: &mut Document,
    start_line: usize,
    end_line: usize,
    outdent: bool,
    prefs: &EditorPrefs,
) {
    let indent = if prefs.use_spaces {
        " ".repeat(prefs.tab_width)
    } else {
        "\t".to_owned()
    };
    let mut lines: Vec<String> = doc.text.split('\n').map(str::to_owned).collect();
    let first = start_line.saturating_sub(1);
    let last = end_line.min(lines.len());
    for line in &mut lines[first..last] {
        if outdent {
            let remove = line
                .chars()
                .take(prefs.tab_width.max(1))
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            if remove > 0 {
                let cut = line
                    .char_indices()
                    .nth(remove)
                    .map(|(i, _)| i)
                    .unwrap_or(line.len());
                line.replace_range(..cut, "");
            }
        } else {
            line.insert_str(0, &indent);
        }
    }
    doc.text = lines.join("\n");
    doc.edited();
    let start = doc.line_start(start_line);
    let end = doc.line_start(end_line)
        + doc.text[doc.line_start(end_line)..]
            .find('\n')
            .unwrap_or(doc.text.len() - doc.line_start(end_line));
    doc.requested_selection = Some((start, end));
}

fn outdent_line(doc: &mut Document, line: usize, prefs: &EditorPrefs) {
    let range = doc.line_range(line);
    let text = doc.text[range.clone()].to_owned();
    let remove = text
        .chars()
        .take(prefs.tab_width.max(1))
        .take_while(|c| *c == ' ' || *c == '\t')
        .count();
    if remove == 0 {
        return;
    }
    let cut = text
        .char_indices()
        .nth(remove)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let column = doc.cursor.column.saturating_sub(remove).max(1);
    doc.text.replace_range(range.start..range.start + cut, "");
    doc.edited();
    let byte = doc.byte_of(line, column);
    doc.requested_selection = Some((byte, byte));
}

fn comment_prefix(language: Language) -> Option<&'static str> {
    match language {
        Language::C
        | Language::Cpp
        | Language::CSharp
        | Language::Java
        | Language::JavaScript
        | Language::TypeScript
        | Language::Rust
        | Language::Go
        | Language::Swift
        | Language::Kotlin
        | Language::Scala
        | Language::Php
        | Language::Css
        | Language::Scss
        | Language::Json
        | Language::Vue
        | Language::Svelte
        | Language::Protobuf => Some("//"),
        Language::Python
        | Language::Ruby
        | Language::Shell
        | Language::Perl
        | Language::PowerShell
        | Language::Yaml
        | Language::Toml
        | Language::Ini
        | Language::Makefile
        | Language::CMake
        | Language::Dockerfile
        | Language::Graphql
        | Language::R
        | Language::Elixir
        | Language::Properties => Some("#"),
        Language::Lua | Language::Sql | Language::Ada | Language::Vhdl | Language::Haskell => {
            Some("--")
        }
        Language::Batch => Some("REM "),
        Language::Tex | Language::Matlab => Some("%"),
        Language::Fortran => Some("!"),
        Language::Lisp | Language::Clojure | Language::Asm => Some(";"),
        _ => None,
    }
}

/// Comment or uncomment the selected lines (or the cursor line).
pub fn toggle_comment(doc: &mut Document) {
    let Some(prefix) = comment_prefix(doc.language) else {
        return;
    };
    let (start_line, end_line) = selected_lines(doc);
    let mut lines: Vec<String> = doc.text.split('\n').map(str::to_owned).collect();
    let first = start_line.saturating_sub(1);
    let last = end_line.min(lines.len());
    let all_commented = lines[first..last]
        .iter()
        .all(|line| line.trim_start().starts_with(prefix));
    for line in &mut lines[first..last] {
        if all_commented {
            if let Some(pos) = line.find(prefix) {
                let after = pos + prefix.len();
                let after = if line[after..].starts_with(' ') {
                    after + 1
                } else {
                    after
                };
                line.replace_range(pos..after, "");
            }
        } else {
            let indent = line.len() - line.trim_start().len();
            let insert = if prefix.ends_with(' ') {
                prefix.to_owned()
            } else {
                format!("{prefix} ")
            };
            line.insert_str(indent, &insert);
        }
    }
    doc.text = lines.join("\n");
    doc.edited();
    let start = doc.line_start(start_line);
    let end = line_end_byte(doc, end_line);
    doc.requested_selection = Some((start, end));
}

/// Duplicate the current line (Ctrl+D).
pub fn duplicate_line(doc: &mut Document) {
    let line = doc.cursor.line.clamp(1, doc.line_count());
    let range = doc.line_range(line);
    let mut content = doc.text[range.clone()].to_owned();
    if !content.ends_with('\n') {
        content.push('\n');
    }
    let insert_at = range.end;
    doc.text.insert_str(insert_at, &content);
    doc.edited();
    let byte = doc.byte_of((line + 1).min(doc.line_count()), doc.cursor.column);
    doc.requested_selection = Some((byte, byte));
}

/// Delete the current line (Ctrl+Shift+K).
pub fn delete_line(doc: &mut Document) {
    let line = doc.cursor.line.clamp(1, doc.line_count());
    let range = doc.line_range(line);
    doc.text.replace_range(range.clone(), "");
    if doc.text.is_empty() {
        doc.text = String::new();
    }
    doc.edited();
    let target = line.min(doc.line_count());
    let byte = doc.byte_of(target, 1);
    doc.requested_selection = Some((byte, byte));
}

/// Move the current line up or down (Alt+Up / Alt+Down).
pub fn move_line(doc: &mut Document, up: bool) {
    let line = doc.cursor.line;
    if (up && line <= 1) || (!up && line >= doc.line_count()) {
        return;
    }
    let other = if up { line - 1 } else { line + 1 };
    let mut lines: Vec<String> = doc.text.split('\n').map(str::to_owned).collect();
    lines.swap(line - 1, other - 1);
    doc.text = lines.join("\n");
    doc.edited();
    let byte = doc.byte_of(other, doc.cursor.column);
    doc.requested_selection = Some((byte, byte));
    doc.scroll_to_line = Some(other);
}

fn selected_lines(doc: &Document) -> (usize, usize) {
    match doc.selection {
        Some((a, b)) if b > a => {
            let start = doc.line_col(a).0;
            let mut end = doc.line_col(b).0;
            if end > start && b == doc.line_start(end) {
                end -= 1;
            }
            (start, end)
        }
        _ => (doc.cursor.line, doc.cursor.line),
    }
}

fn line_end_byte(doc: &Document, line: usize) -> usize {
    let range = doc.line_range(line);
    let mut end = range.end;
    if end > range.start && doc.text.as_bytes().get(end.saturating_sub(1)) == Some(&b'\n') {
        end -= 1;
    }
    end
}

/// Go to a 1-based line, placing the caret at its start.
pub fn goto_line(doc: &mut Document, line: usize) {
    let line = line.clamp(1, doc.line_count());
    let byte = doc.line_start(line);
    doc.requested_selection = Some((byte, byte));
    doc.scroll_to_line = Some(line);
}

/// Select the whole document (Ctrl+A equivalent for menu).
pub fn select_all(doc: &mut Document) {
    doc.requested_selection = Some((0, doc.text.len()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    fn doc_with(text: &str) -> Document {
        let mut doc = Document::untitled(1, 1);
        doc.set_text(text.to_owned());
        doc.cursor.line = 1;
        doc
    }

    #[test]
    fn toggle_comment_adds_and_removes() {
        let mut doc = doc_with("fn main() {}\n");
        doc.cursor.line = 1;
        doc.language = Language::Rust;
        toggle_comment(&mut doc);
        assert_eq!(doc.text, "// fn main() {}\n");
        doc.cursor.line = 1;
        toggle_comment(&mut doc);
        assert_eq!(doc.text, "fn main() {}\n");
    }

    #[test]
    fn duplicate_and_delete_line() {
        let mut doc = doc_with("one\ntwo\n");
        doc.cursor.line = 1;
        duplicate_line(&mut doc);
        assert_eq!(doc.text, "one\none\ntwo\n");
        doc.cursor.line = 2;
        delete_line(&mut doc);
        assert_eq!(doc.text, "one\ntwo\n");
    }

    #[test]
    fn move_line_swaps_neighbours() {
        let mut doc = doc_with("a\nb\nc");
        doc.cursor.line = 2;
        move_line(&mut doc, true);
        assert_eq!(doc.text, "b\na\nc");
        doc.cursor.line = 2;
        move_line(&mut doc, false);
        assert_eq!(doc.text, "b\nc\na");
    }

    #[test]
    fn indent_and_outdent_lines() {
        let mut doc = doc_with("a\nb\n");
        doc.selection = Some((0, 3));
        let prefs = EditorPrefs::default();
        indent_lines(&mut doc, 1, 2, false, &prefs);
        assert_eq!(doc.text, "    a\n    b\n");
        doc.selection = Some((0, 11));
        indent_lines(&mut doc, 1, 2, true, &prefs);
        assert_eq!(doc.text, "a\nb\n");
    }

    #[test]
    fn goto_line_sets_selection() {
        let mut doc = doc_with("a\nb\nc");
        goto_line(&mut doc, 2);
        assert_eq!(doc.requested_selection, Some((2, 2)));
        assert_eq!(doc.scroll_to_line, Some(2));
    }

    #[test]
    fn select_all_then_delete_clears_the_buffer() {
        let mut doc = doc_with("hello world");
        doc.selection = Some((0, doc.text.len()));
        replace_selection(&mut doc, "");
        assert_eq!(doc.text, "");
        assert_eq!(doc.cursor.line, 1);
        assert_eq!(doc.cursor.column, 1);
        assert_eq!(doc.selection, Some((0, 0)));
    }

    #[test]
    fn stale_selection_offsets_do_not_panic() {
        let mut doc = doc_with("short");
        // Offsets from the previous frame, longer than the text and not on a
        // character boundary: this is what took the window down.
        doc.update_cursor(5, Some((3, 999)));
        assert_eq!(doc.cursor.selection_chars, 2);
        doc.update_cursor(999, Some((0, 999)));
        assert_eq!(doc.cursor.column, 6);
    }

    #[test]
    fn multibyte_offsets_are_snapped() {
        let mut doc = doc_with("привет");
        // 1 is inside the first two-byte character, so it snaps back to 0.
        doc.update_cursor(1, Some((1, 2)));
        assert_eq!(doc.cursor.column, 1);
        assert_eq!(doc.cursor.selection_chars, 1);
    }

    /// Drives the real widget headlessly: type, Ctrl+A, Backspace.
    #[test]
    fn ctrl_a_then_backspace_clears_without_closing() {
        let ctx = egui::Context::default();
        let theme = crate::theme::Theme::new(true, Color32::from_rgb(103, 80, 164), 1.0, 14.0);
        let prefs = EditorPrefs::default();
        let mut cache = HighlightCache::default();
        let mut doc = doc_with("hello world");

        let mut frame = |ctx: &egui::Context, doc: &mut Document, events: Vec<egui::Event>| {
            let mut input = egui::RawInput::default();
            input.events = events;
            let mut cache_local = std::mem::take(&mut cache);
            ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, &theme, doc, &prefs, &[], None, &mut cache_local);
                });
            });
            cache = cache_local;
        };

        let key = |key: Key, modifiers: egui::Modifiers| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        };

        // First frame focuses the widget (the app does this on document change).
        let id = egui::Id::new(("editor", doc.id));
        ctx.memory_mut(|memory| memory.request_focus(id));
        frame(&ctx, &mut doc, vec![]);
        ctx.memory_mut(|memory| memory.request_focus(id));
        frame(&ctx, &mut doc, vec![]);

        // Ctrl+A selects everything.
        frame(&ctx, &mut doc, vec![key(Key::A, egui::Modifiers::CTRL)]);
        assert_eq!(
            doc.selection,
            Some((0, doc.text.len())),
            "Ctrl+A did not select the buffer"
        );
        frame(&ctx, &mut doc, vec![]);

        // Backspace deletes the selection; the buffer must end up empty and the
        // app must still be running (a panic here is the reported crash).
        frame(
            &ctx,
            &mut doc,
            vec![key(Key::Backspace, egui::Modifiers::NONE)],
        );
        frame(&ctx, &mut doc, vec![]);

        assert_eq!(doc.text, "", "Backspace did not clear the selection");
        assert_eq!(doc.cursor.line, 1);
        assert_eq!(doc.cursor.column, 1);
    }
}
