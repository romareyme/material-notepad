//! The editor surface: syntax-coloured text, gutter, overlays and commands.
//!
//! Text input, selection, IME and clipboard handling come from egui's
//! `TextEdit`; this module supplies the Material look (gutter, current line,
//! find highlights) and the editor commands Notepad Next exposes.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::text_edit::TextEditState;
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
    spans: Arc<Vec<Span>>,
}

#[derive(Default)]
pub struct HighlightCache {
    entries: HashMap<u64, CacheEntry>,
    order: Vec<u64>,
}

impl HighlightCache {
    fn spans(&mut self, doc: &Document) -> Arc<Vec<Span>> {
        if let Some(entry) = self.entries.get(&doc.id) {
            if entry.revision == doc.revision && entry.language == doc.language {
                return entry.spans.clone();
            }
        }
        let spans = if doc.text.len() > MAX_HIGHLIGHT_BYTES {
            Arc::new(Vec::new())
        } else {
            Arc::new(highlight(&doc.text, doc.language))
        };
        self.entries.insert(
            doc.id,
            CacheEntry {
                revision: doc.revision,
                language: doc.language,
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
        let start = span.range.start;
        let end = span.range.end;
        if start < pos
            || start >= end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            continue;
        }
        if start > pos {
            job.append(&text[pos..start], 0.0, base.clone());
        }
        let format = TextFormat {
            color: span.kind.color(&colors),
            ..base.clone()
        };
        job.append(&text[start..end], 0.0, format);
        pos = end;
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
        let digits = {
            let mut count = 1usize;
            let mut lines = doc.line_count();
            while lines >= 10 {
                lines /= 10;
                count += 1;
            }
            count
        };
        (digit_width * digits as f32 + 12.0).max(30.0)
    } else {
        0.0
    };
    let apply_selection = |ui: &Ui, doc: &mut Document| apply_requested_selection(ui, id, doc);
    let mut selection_requested = apply_selection(ui, doc);

    let mut event = EditorEvent::default();

    if !doc.multi_cursors.is_empty() {
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            doc.multi_cursors.clear();
        } else if handle_multi_cursor_input(ui, doc) {
            event.changed = true;
            selection_requested = true;
        }
    }

    // Editor shortcuts apply to the active document whether or not the widget
    // currently owns keyboard focus (a menu or dialog can take it).
    handle_keys(ui, doc, prefs);
    selection_requested |= apply_selection(ui, doc);

    let may_edit_text = ui.input(|i| {
        i.events.iter().any(|ev| {
            matches!(
                ev,
                egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Cut
                    | egui::Event::Key {
                        key: Key::Backspace | Key::Delete,
                        pressed: true,
                        ..
                    }
            )
        })
    });
    let undo_snapshot = if may_edit_text {
        Some((doc.text.clone(), caret_byte(doc)))
    } else {
        None
    };

    let spans = cache.spans(doc);
    let language = doc.language;
    let initial_len = doc.text.len();
    let mut first_layout = true;
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

    let mut scroll_to_line = doc.scroll_to_line.take();
    let viewport_rows = ((ui.available_height() / theme.editor_line_height).ceil() as usize).max(6);

    ScrollArea::both()
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
                // TextEdit re-runs the layouter immediately after mutating the buffer
                // (for example, Ctrl+A followed by Backspace); using the pre-edit
                // spans would slice out of bounds and panic.
                let job = if first_layout && text.len() == initial_len {
                    first_layout = false;
                    layout_job(text, &spans, theme, wrap)
                } else {
                    first_layout = false;
                    let fresh = if text.len() > MAX_HIGHLIGHT_BYTES {
                        Vec::new()
                    } else {
                        highlight(text, language)
                    };
                    layout_job(text, &fresh, theme, wrap)
                };
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
                doc.selection_sticky = false;
                event.changed = true;
                if let Some((before, c_before)) = &undo_snapshot {
                    let c_after = output
                        .cursor_range
                        .map(|r| doc.byte_index(r.primary.ccursor.index))
                        .unwrap_or_else(|| caret_byte(doc));
                    doc.record_change(before, *c_before, c_after);
                }
            }

            if let Some(range) = output.cursor_range {
                let primary = range.primary.ccursor.index;
                let secondary = range.secondary.ccursor.index;
                let a = doc.byte_index(primary);
                let b = doc.byte_index(secondary);
                // Alt+Click adds another cursor to multi_cursors.
                if output.response.clicked() && ui.input(|i| i.modifiers.alt) {
                    if doc.multi_cursors.is_empty() {
                        let prev = doc.selection.unwrap_or((a, a));
                        doc.multi_cursors.push(prev);
                    }
                    if !doc.multi_cursors.contains(&(a, a)) {
                        doc.multi_cursors.push((a, a));
                        doc.multi_cursors.sort_by_key(|&(s, _)| s);
                    }
                } else if output.response.clicked() && !ui.input(|i| i.modifiers.alt) {
                    doc.multi_cursors.clear();
                }
                // A selection this frame set by a command wins over the widget's
                // report for one frame, which still shows the previous caret.
                // A sticky selection from Ctrl+A keeps winning until the user
                // types or moves, because the widget reports its stale caret.
                if (!selection_requested && !doc.selection_sticky) || output.response.changed() {
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
                if deleting && !event.changed {
                    if let Some((start, end)) = selected {
                        // The widget may already have deleted the text this
                        // frame, leaving these offsets past the end. Clamping
                        // them is what keeps the window alive.
                        let start = doc.snap(start);
                        let end = doc.snap(end).max(start);
                        if end > start {
                            let before = doc.text.clone();
                            doc.text.replace_range(start..end, "");
                            doc.edited();
                            doc.record_change(&before, end, start);
                            event.changed = true;
                            doc.selection = Some((start, start));
                            doc.requested_selection = Some((start, start));
                            doc.selection_sticky = false;
                            doc.update_cursor(start, Some((start, start)));
                            apply_selection(ui, doc);
                        }
                    }
                }
                // Any user input that moves the caret clears a command-made
                // selection: typing, caret keys, and mouse clicks or drags.
                // (response.changed() also fires when the state is stored, so
                // it is not a usable signal here.)
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
                        egui::Event::PointerButton {
                            pressed: true,
                            button: egui::PointerButton::Primary,
                            ..
                        } => true,
                        egui::Event::PointerGone => true,
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

            // Current line + indent guides + brackets + HEX swatches + find matches + multi-cursors.
            let painter = ui.painter_at(output.text_clip_rect);
            let origin = output.galley_pos;
            let clip = output.text_clip_rect;
            if let Some(rect) = line_rect(&output.galley, origin, doc.cursor.line) {
                painter.rect_filled(rect, 0.0, line_highlight);
            }
            paint_indent_guides(
                &painter,
                &output.galley,
                origin,
                clip,
                doc,
                prefs.tab_width,
                digit_width,
                theme.scheme.outline_variant,
            );
            if let Some((open_b, close_b)) = matching_bracket(&doc.text, caret_byte(doc)) {
                let bracket_bg = Color32::from_rgba_unmultiplied(
                    theme.scheme.secondary.r(),
                    theme.scheme.secondary.g(),
                    theme.scheme.secondary.b(),
                    55,
                );
                for pos in [open_b, close_b] {
                    if let Some(rect) = range_rect(doc, &output.galley, origin, &(pos..pos + 1)) {
                        painter.rect_filled(rect, 2.0, bracket_bg);
                        painter.rect_stroke(
                            rect,
                            2.0,
                            Stroke::new(1.0_f32, theme.scheme.primary),
                        );
                    }
                }
            }
            paint_hex_swatches(&painter, doc, &output.galley, origin, clip);
            for (index, range) in matches.iter().enumerate() {
                if let Some(rect) = range_rect(doc, &output.galley, origin, range) {
                    if !clip.intersects(rect.expand(2.0)) {
                        continue;
                    }
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
            if !doc.multi_cursors.is_empty() {
                paint_multi_cursors(&painter, doc, &output.galley, origin, theme);
            }

            if let Some(line) = scroll_to_line.take() {
                if let Some(rect) = line_rect(&output.galley, origin, line) {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
            }
        });

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
    let clip = ui.clip_rect();
    let mut line_no = 1usize;
    let mut first_row = true;
    for row in &output.galley.rows {
        let row_top = origin.y + row.rect.min.y;
        let row_bottom = origin.y + row.rect.max.y;
        if row_top > clip.bottom() {
            break;
        }
        if first_row && row_bottom >= clip.top() {
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
    // If the match wraps onto another row, clamp to the first row instead of
    // stretching a rectangle across the editor width.
    let mut rect = if (a.min.y - b.min.y).abs() < 1.0 && b.max.x >= a.min.x {
        Rect::from_min_max(a.min, b.max)
    } else {
        Rect::from_min_max(a.min, Pos2::new((a.min.x + 12.0).max(a.max.x), a.max.y))
    };
    rect = rect.translate(origin.to_vec2());
    if rect.width() < 2.0 {
        rect.max.x = rect.min.x + 2.0;
    }
    Some(rect)
}

fn handle_keys(ui: &mut Ui, doc: &mut Document, prefs: &EditorPrefs) {
    // egui 0.27 has no "select all" for TextEdit, so Ctrl+A is handled here.
    // The modifiers come from the event itself: input.modifiers can lag a frame.
    let select_all_pressed = ui.input(|i| {
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
    if select_all_pressed {
        select_all(doc);
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
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
    let (start, end) = match doc.selection {
        Some((a, b)) => (a.min(b).min(doc.text.len()), a.max(b).min(doc.text.len())),
        None => (cursor_before, cursor_before),
    };
    doc.text.replace_range(start..end, text);
    doc.edited();
    let caret = start + text.len();
    doc.record_change(&before, cursor_before, caret);
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
    let block_start = doc.line_start(start_line);
    let block_end = line_end_byte(doc, end_line);
    let mut lines: Vec<String> = doc.text[block_start..block_end]
        .split('\n')
        .map(str::to_owned)
        .collect();
    for line in &mut lines {
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
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
    let replaced = lines.join("\n");
    doc.text.replace_range(block_start..block_end, &replaced);
    doc.edited();
    let start = doc.line_start(start_line);
    let end = line_end_byte(doc, end_line);
    doc.record_change(&before, cursor_before, end);
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
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
    let cut = text
        .char_indices()
        .nth(remove)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let column = doc.cursor.column.saturating_sub(remove).max(1);
    doc.text.replace_range(range.start..range.start + cut, "");
    doc.edited();
    let byte = doc.byte_of(line, column);
    doc.record_change(&before, cursor_before, byte);
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
    let block_start = doc.line_start(start_line);
    let block_end = line_end_byte(doc, end_line);
    let mut lines: Vec<String> = doc.text[block_start..block_end]
        .split('\n')
        .map(str::to_owned)
        .collect();
    let has_non_empty = lines.iter().any(|line| !line.trim().is_empty());
    let all_commented = if has_non_empty {
        lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .all(|line| line.trim_start().starts_with(prefix))
    } else {
        lines
            .iter()
            .all(|line| line.trim_start().starts_with(prefix))
    };
    for line in &mut lines {
        if has_non_empty && line.trim().is_empty() {
            continue;
        }
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
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
    let replaced = lines.join("\n");
    doc.text.replace_range(block_start..block_end, &replaced);
    doc.edited();
    let start = doc.line_start(start_line);
    let end = line_end_byte(doc, end_line);
    doc.record_change(&before, cursor_before, end);
    doc.requested_selection = Some((start, end));
}

/// Duplicate the current line (Ctrl+D).
pub fn duplicate_line(doc: &mut Document) {
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
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
    doc.record_change(&before, cursor_before, byte);
    doc.requested_selection = Some((byte, byte));
}

/// Delete the current line (Ctrl+Shift+K).
pub fn delete_line(doc: &mut Document) {
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
    let line = doc.cursor.line.clamp(1, doc.line_count());
    let range = doc.line_range(line);
    doc.text.replace_range(range.clone(), "");
    doc.edited();
    let target = line.min(doc.line_count());
    let byte = doc.byte_of(target, 1);
    doc.record_change(&before, cursor_before, byte);
    doc.requested_selection = Some((byte, byte));
}

/// Move the current line up or down (Alt+Up / Alt+Down).
pub fn move_line(doc: &mut Document, up: bool) {
    let line = doc.cursor.line;
    if (up && line <= 1) || (!up && line >= doc.line_count()) {
        return;
    }
    let before = doc.text.clone();
    let cursor_before = caret_byte(doc);
    let other = if up { line - 1 } else { line + 1 };
    let (first, second) = if up { (other, line) } else { (line, other) };
    let first_start = doc.line_start(first);
    let first_end = line_end_byte(doc, first);
    let second_start = doc.line_start(second);
    let second_end = line_end_byte(doc, second);
    let swapped = format!(
        "{}\n{}",
        &doc.text[second_start..second_end],
        &doc.text[first_start..first_end]
    );
    doc.text.replace_range(first_start..second_end, &swapped);
    doc.edited();
    let byte = doc.byte_of(other, doc.cursor.column);
    doc.record_change(&before, cursor_before, byte);
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

/// Select the whole document (Ctrl+A and the menu entry).
pub fn select_all(doc: &mut Document) {
    let len = doc.text.len();
    doc.multi_cursors.clear();
    doc.selection = Some((0, len));
    doc.requested_selection = Some((0, len));
    doc.selection_sticky = true;
    doc.update_cursor(len, Some((0, len)));
}

/// Select all occurrences of the current selection (or word at caret) as multi-cursors.
pub fn select_all_occurrences(doc: &mut Document) -> usize {
    let needle = match doc.selection {
        Some((a, b)) if b > a => doc.text[doc.snap(a)..doc.snap(b)].to_owned(),
        _ => word_at(doc, caret_byte(doc)),
    };
    if needle.is_empty() {
        return 0;
    }
    let mut ranges = Vec::new();
    let mut pos = 0usize;
    while let Some(found) = doc.text[pos..].find(&needle) {
        let start = pos + found;
        let end = start + needle.len();
        ranges.push((start, end));
        pos = end;
        if ranges.len() >= 500 {
            break;
        }
    }
    if ranges.len() > 1 {
        doc.multi_cursors = ranges;
        doc.multi_cursors.len()
    } else {
        if let Some((s, e)) = ranges.first().copied() {
            doc.requested_selection = Some((s, e));
            doc.selection = Some((s, e));
        }
        ranges.len()
    }
}

/// Add another cursor on the line above or below at the same column.
pub fn add_cursor_vertical(doc: &mut Document, down: bool) {
    if doc.multi_cursors.is_empty() {
        let pos = caret_byte(doc);
        doc.multi_cursors.push((pos, pos));
    }
    let anchor = if down {
        doc.multi_cursors.last().map(|&(_, b)| b).unwrap_or(0)
    } else {
        doc.multi_cursors.first().map(|&(a, _)| a).unwrap_or(0)
    };
    let (line, col) = doc.line_col(anchor);
    let target = if down {
        (line + 1).min(doc.line_count())
    } else {
        line.saturating_sub(1).max(1)
    };
    if target == line {
        return;
    }
    let byte = doc.byte_of(target, col);
    if !doc.multi_cursors.contains(&(byte, byte)) {
        doc.multi_cursors.push((byte, byte));
        doc.multi_cursors.sort_by_key(|&(s, _)| s);
    }
}

fn word_at(doc: &Document, byte: usize) -> String {
    let byte = doc.snap(byte);
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut start = byte;
    for (i, ch) in doc.text[..byte].char_indices().rev() {
        if is_word(ch) {
            start = i;
        } else {
            break;
        }
    }
    let mut end = byte;
    for (i, ch) in doc.text[byte..].char_indices() {
        if is_word(ch) {
            end = byte + i + ch.len_utf8();
        } else {
            break;
        }
    }
    doc.text[start..end].to_owned()
}

fn handle_multi_cursor_input(ui: &mut Ui, doc: &mut Document) -> bool {
    enum Op {
        Insert(String),
        Backspace,
        Delete,
    }
    let mut op: Option<Op> = None;
    ui.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Backspace) {
            op = Some(Op::Backspace);
        } else if i.consume_key(Modifiers::NONE, Key::Delete) {
            op = Some(Op::Delete);
        } else if i.consume_key(Modifiers::NONE, Key::Enter) {
            op = Some(Op::Insert("\n".to_owned()));
        } else {
            let mut text_buf = String::new();
            i.events.retain(|ev| {
                if let egui::Event::Text(t) = ev {
                    if !t.is_empty() {
                        text_buf.push_str(t);
                        return false;
                    }
                }
                true
            });
            if !text_buf.is_empty() {
                op = Some(Op::Insert(text_buf));
            }
        }
    });

    let Some(op) = op else {
        return false;
    };

    let before = doc.text.clone();
    let cursor_before = doc.multi_cursors.first().map(|c| c.0).unwrap_or(0);
    let mut cursors = std::mem::take(&mut doc.multi_cursors);
    cursors.sort_by_key(|&(a, _)| a);

    // Apply edits from the back so earlier byte offsets stay valid, then shift
    // cumulative deltas for the updated cursor positions.
    let mut edits: Vec<(usize, usize, String)> = Vec::with_capacity(cursors.len());
    for &(raw_a, raw_b) in &cursors {
        let a = doc.snap(raw_a.min(raw_b));
        let b = doc.snap(raw_a.max(raw_b));
        match &op {
            Op::Insert(s) => edits.push((a, b, s.clone())),
            Op::Backspace => {
                if b > a {
                    edits.push((a, b, String::new()));
                } else if a > 0 {
                    let prev = doc.text[..a]
                        .char_indices()
                        .next_back()
                        .map(|(idx, _)| idx)
                        .unwrap_or(0);
                    edits.push((prev, a, String::new()));
                } else {
                    edits.push((0, 0, String::new()));
                }
            }
            Op::Delete => {
                if b > a {
                    edits.push((a, b, String::new()));
                } else if b < doc.text.len() {
                    let next = doc.text[b..]
                        .chars()
                        .next()
                        .map(|c| b + c.len_utf8())
                        .unwrap_or(b);
                    edits.push((b, next, String::new()));
                } else {
                    edits.push((b, b, String::new()));
                }
            }
        }
    }

    for (start, end, rep) in edits.iter().rev() {
        doc.text.replace_range(*start..*end, rep);
    }

    let mut delta: isize = 0;
    let mut updated = Vec::with_capacity(edits.len());
    for (start, end, rep) in edits {
        let new_pos = ((start as isize) + delta + rep.len() as isize).max(0) as usize;
        delta += rep.len() as isize - (end - start) as isize;
        let snapped = doc.snap(new_pos);
        updated.push((snapped, snapped));
    }
    doc.multi_cursors = updated;
    doc.edited();
    let first = doc.multi_cursors.first().map(|c| c.0).unwrap_or(0);
    doc.record_change(&before, cursor_before, first);
    doc.requested_selection = Some((first, first));
    doc.selection = Some((first, first));
    doc.update_cursor(first, None);
    true
}

fn paint_multi_cursors(
    painter: &egui::Painter,
    doc: &Document,
    galley: &egui::Galley,
    origin: Pos2,
    theme: &Theme,
) {
    let sel_fill = Color32::from_rgba_unmultiplied(
        theme.scheme.primary.r(),
        theme.scheme.primary.g(),
        theme.scheme.primary.b(),
        70,
    );
    for &(a, b) in &doc.multi_cursors {
        let a = doc.snap(a);
        let b = doc.snap(b);
        if b > a {
            if let Some(rect) = range_rect(doc, galley, origin, &(a..b)) {
                painter.rect_filled(rect, 2.0, sel_fill);
            }
        }
        let caret_pos = galley
            .pos_from_ccursor(CCursor::new(doc.char_index(b)))
            .translate(origin.to_vec2());
        painter.line_segment(
            [
                Pos2::new(caret_pos.min.x, caret_pos.min.y),
                Pos2::new(caret_pos.min.x, caret_pos.max.y),
            ],
            Stroke::new(2.0_f32, theme.scheme.primary),
        );
    }
}

/// Find the matching `()`, `[]`, or `{}` partner for a bracket at or just before `byte`.
pub fn matching_bracket(text: &str, byte: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let probe = |pos: usize| -> Option<(usize, u8, u8, bool)> {
        let b = *bytes.get(pos)?;
        match b {
            b'(' => Some((pos, b'(', b')', true)),
            b'[' => Some((pos, b'[', b']', true)),
            b'{' => Some((pos, b'{', b'}', true)),
            b')' => Some((pos, b')', b'(', false)),
            b']' => Some((pos, b']', b'[', false)),
            b'}' => Some((pos, b'}', b'{', false)),
            _ => None,
        }
    };
    let (pos, open, close, forward) = probe(byte).or_else(|| {
        byte.checked_sub(1).and_then(probe)
    })?;

    const MAX_SCAN: usize = 64 * 1024;
    let mut depth = 0usize;
    if forward {
        let limit = (pos + MAX_SCAN).min(bytes.len());
        for (i, &b) in bytes[pos..limit].iter().enumerate() {
            if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth == 0 {
                    return Some((pos, pos + i));
                }
            }
        }
    } else {
        let start = pos.saturating_sub(MAX_SCAN);
        for i in (start..=pos).rev() {
            let b = bytes[i];
            if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth == 0 {
                    return Some((i, pos));
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn paint_indent_guides(
    painter: &egui::Painter,
    galley: &egui::Galley,
    origin: Pos2,
    clip: Rect,
    doc: &Document,
    tab_width: usize,
    char_width: f32,
    color: Color32,
) {
    let step = tab_width.max(1);
    let guide_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70);
    let mut line_no = 1usize;
    let mut first_row = true;
    for row in &galley.rows {
        let row_top = origin.y + row.rect.min.y;
        let row_bottom = origin.y + row.rect.max.y;
        if row_top > clip.bottom() {
            break;
        }
        if first_row && row_bottom >= clip.top() {
            let range = doc.line_range(line_no);
            let mut cols = 0usize;
            for b in doc.text[range].bytes() {
                match b {
                    b' ' => cols += 1,
                    b'\t' => cols += step,
                    _ => break,
                }
            }
            let mut k = step;
            while k <= cols {
                let x = origin.x + k as f32 * char_width;
                painter.line_segment(
                    [Pos2::new(x, row_top), Pos2::new(x, row_bottom)],
                    Stroke::new(1.0_f32, guide_color),
                );
                k += step;
            }
        }
        first_row = row.ends_with_newline;
        if row.ends_with_newline {
            line_no += 1;
        }
    }
}

fn paint_hex_swatches(
    painter: &egui::Painter,
    doc: &Document,
    galley: &egui::Galley,
    origin: Pos2,
    clip: Rect,
) {
    // Only scan the lines currently visible in the viewport.
    let mut first_visible = None;
    let mut last_visible = 1usize;
    let mut line_no = 1usize;
    let mut first_row = true;
    for row in &galley.rows {
        let row_top = origin.y + row.rect.min.y;
        let row_bottom = origin.y + row.rect.max.y;
        if row_top > clip.bottom() {
            break;
        }
        if first_row && row_bottom >= clip.top() {
            if first_visible.is_none() {
                first_visible = Some(line_no);
            }
            last_visible = line_no;
        }
        first_row = row.ends_with_newline;
        if row.ends_with_newline {
            line_no += 1;
        }
    }
    let Some(start_line) = first_visible else {
        return;
    };

    for line in start_line..=last_visible {
        let range = doc.line_range(line);
        let bytes = doc.text[range.clone()].as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'#' {
                let hex_start = i + 1;
                let mut hex_end = hex_start;
                while hex_end < bytes.len() && bytes[hex_end].is_ascii_hexdigit() {
                    hex_end += 1;
                }
                let hex_len = hex_end - hex_start;
                let next_ok = bytes
                    .get(hex_end)
                    .map(|b| !b.is_ascii_alphanumeric() && *b != b'_')
                    .unwrap_or(true);
                if next_ok && matches!(hex_len, 3 | 6 | 8) {
                    if let Some(color) = parse_hex_color(&bytes[hex_start..hex_end]) {
                        let abs = (range.start + i)..(range.start + hex_end);
                        if let Some(rect) = range_rect(doc, galley, origin, &abs) {
                            let bar = Rect::from_min_max(
                                Pos2::new(rect.left(), rect.bottom() - 2.5),
                                Pos2::new(rect.right(), rect.bottom()),
                            );
                            painter.rect_filled(bar, 1.0, color);
                        }
                    }
                }
                i = hex_end.max(i + 1);
            } else {
                i += 1;
            }
        }
    }
}

fn parse_hex_color(hex: &[u8]) -> Option<Color32> {
    let val = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    let byte2 = |hi: u8, lo: u8| Some(val(hi)? * 16 + val(lo)?);
    match hex.len() {
        3 => {
            let r = val(hex[0])? * 17;
            let g = val(hex[1])? * 17;
            let b = val(hex[2])? * 17;
            Some(Color32::from_rgb(r, g, b))
        }
        6 => Some(Color32::from_rgb(
            byte2(hex[0], hex[1])?,
            byte2(hex[2], hex[3])?,
            byte2(hex[4], hex[5])?,
        )),
        8 => Some(Color32::from_rgba_unmultiplied(
            byte2(hex[0], hex[1])?,
            byte2(hex[2], hex[3])?,
            byte2(hex[4], hex[5])?,
            byte2(hex[6], hex[7])?,
        )),
        _ => None,
    }
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
        let mut doc = doc_with("РїСЂРёРІРµС‚");
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
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let mut cache_local = std::mem::take(&mut cache);
            let _ = ctx.run(input, |ctx| {
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

    /// Ctrl+A, then a mouse click that moves the caret, then Backspace: the
    /// buffer must lose one character, not everything.
    #[test]
    fn ctrl_a_click_backspace_deletes_one_character() {
        let ctx = egui::Context::default();
        let theme = crate::theme::Theme::new(true, Color32::from_rgb(103, 80, 164), 1.0, 14.0);
        let prefs = EditorPrefs::default();
        let mut cache = HighlightCache::default();
        let mut doc = doc_with("hello world");

        let mut frame = |ctx: &egui::Context, doc: &mut Document, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let mut cache_local = std::mem::take(&mut cache);
            let _ = ctx.run(input, |ctx| {
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

        let id = egui::Id::new(("editor", doc.id));
        ctx.memory_mut(|memory| memory.request_focus(id));
        frame(&ctx, &mut doc, vec![]);
        frame(&ctx, &mut doc, vec![]);

        frame(&ctx, &mut doc, vec![key(Key::A, egui::Modifiers::CTRL)]);
        frame(&ctx, &mut doc, vec![]);

        // A click into the text drops the command selection.
        let pos = egui::Pos2::new(200.0, 60.0);
        frame(
            &ctx,
            &mut doc,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerGone,
            ],
        );
        frame(&ctx, &mut doc, vec![]);
        assert!(
            !doc.selection_sticky,
            "click did not clear the sticky selection"
        );

        frame(
            &ctx,
            &mut doc,
            vec![key(Key::Backspace, egui::Modifiers::NONE)],
        );
        frame(&ctx, &mut doc, vec![]);
        assert_eq!(
            doc.text.len(),
            "hello world".len() - 1,
            "Backspace deleted more than the character at the caret"
        );
    }

    #[test]
    fn matching_bracket_and_hex_color() {
        assert_eq!(matching_bracket("fn f() { a[0] }", 7), Some((7, 14)));
        assert_eq!(matching_bracket("fn f() { a[0] }", 14), Some((7, 14)));
        assert_eq!(parse_hex_color(b"6750A4"), Some(Color32::from_rgb(0x67, 0x50, 0xA4)));
        assert_eq!(parse_hex_color(b"f0a"), Some(Color32::from_rgb(0xFF, 0x00, 0xAA)));
    }

    #[test]
    fn select_all_occurrences_creates_multi_cursors() {
        let mut doc = doc_with("foo bar foo baz foo");
        doc.selection = Some((0, 3));
        assert_eq!(select_all_occurrences(&mut doc), 3);
        assert_eq!(doc.multi_cursors, vec![(0, 3), (8, 11), (16, 19)]);
    }

    #[test]
    fn ctrl_a_then_backspace_on_highlighted_doc_clears_safely() {
        let ctx = egui::Context::default();
        let theme = crate::theme::Theme::new(true, Color32::from_rgb(103, 80, 164), 1.0, 14.0);
        let prefs = EditorPrefs::default();
        let mut cache = HighlightCache::default();
        let mut doc = doc_with("fn main() {\n    println!(\"Привет\");\n}\n");
        doc.language = Language::Rust;

        let mut frame = |doc: &mut Document, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, &theme, doc, &prefs, &[], None, &mut cache);
                });
            });
        };

        let key = |key: Key, modifiers: egui::Modifiers| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        };

        let id = egui::Id::new(("editor", doc.id));
        ctx.memory_mut(|memory| memory.request_focus(id));
        frame(&mut doc, vec![]);
        frame(&mut doc, vec![key(Key::A, egui::Modifiers::CTRL)]);
        frame(&mut doc, vec![key(Key::Backspace, egui::Modifiers::NONE)]);
        frame(&mut doc, vec![]);

        assert_eq!(doc.text, "");
        assert_eq!(doc.cursor.line, 1);
        assert_eq!(doc.cursor.column, 1);
    }
}
