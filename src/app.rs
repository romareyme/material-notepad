//! Application shell: Material You chrome, document management, commands.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, Frame, Id, Key, Layout, Margin, Modifiers, Order, Pos2, Rect,
    RichText, ScrollArea, Sense, Stroke, Vec2,
};

use crate::document::{CursorInfo, Document, Encoding, Eol};
use crate::editor::{self, EditorPrefs, HighlightCache};
use crate::files::{find_in_files, FileHit, FileTree};
use crate::highlight::Language;
use crate::preview;
use crate::search::{next_match, Finder, SearchOptions};
use crate::session::{self, Session, SessionFile, Settings};
use crate::theme::{Theme, SEEDS};
use crate::transform;
use crate::widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Files,
    Search,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    Main,
    Encoding,
    Eol,
    Language,
    Tab,
}

enum Dialog {
    Settings { draft: Settings },
    About,
    Shortcuts,
    GoToLine { value: String },
    ConfirmClose { index: usize },
    Error { title: String, message: String },
    CommandPalette { query: String, selected: usize, focus: bool },
    PickDiff { other: usize },
    Stats,
}

struct Snackbar {
    text: String,
    created: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    New,
    Open,
    OpenFolder,
    Save,
    SaveAs,
    Reload,
    CloseTab,
    CloseAll,
    Exit,
    Undo,
    Redo,
    DuplicateLine,
    DeleteLine,
    ToggleComment,
    MoveLineUp,
    MoveLineDown,
    SelectAll,
    SelectAllOccurrences,
    AddCursorAbove,
    AddCursorBelow,
    Find,
    Replace,
    FindNext,
    FindPrev,
    GoToLine,
    FindInFiles,
    ToggleWrap,
    ToggleLineNumbers,
    ToggleZenMode,
    ToggleAlwaysOnTop,
    TogglePreview,
    CompareTabs,
    TogglePinTab,
    ShowStats,
    CommandPalette,
    SortLinesAsc,
    SortLinesDesc,
    RemoveDuplicateLines,
    RemoveEmptyLines,
    TrimTrailingSpaces,
    Uppercase,
    Lowercase,
    TitleCase,
    ToggleSnakeCamel,
    FormatJson,
    MinifyJson,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Settings,
    About,
    Shortcuts,
    NextTab,
    PrevTab,
    Menu,
}

struct FindState {
    open: bool,
    replace: bool,
    query: String,
    replacement: String,
    options: SearchOptions,
    matches: Vec<std::ops::Range<usize>>,
    current: Option<usize>,
    error: Option<String>,
    focus_query: bool,
    finder: Finder,
    last_query: String,
    last_options: SearchOptions,
    last_revision: u64,
    last_doc: u64,
}

impl FindState {
    fn new() -> Self {
        Self {
            open: false,
            replace: false,
            query: String::new(),
            replacement: String::new(),
            options: SearchOptions::default(),
            matches: Vec::new(),
            current: None,
            error: None,
            focus_query: false,
            finder: Finder::new(),
            last_query: String::new(),
            last_options: SearchOptions::default(),
            last_revision: 0,
            last_doc: 0,
        }
    }

    fn refresh(&mut self, doc: &Document) {
        if !self.open {
            return;
        }
        if self.last_query == self.query
            && self.last_options == self.options
            && self.last_revision == doc.revision
            && self.last_doc == doc.id
        {
            return;
        }
        self.matches = self.finder.matches(&doc.text, &self.query, &self.options);
        self.error = self.finder.error().map(str::to_owned);
        self.last_query = self.query.clone();
        self.last_options = self.options.clone();
        self.last_revision = doc.revision;
        self.last_doc = doc.id;
        if self.matches.is_empty() {
            self.current = None;
        } else if self
            .current
            .map(|c| c >= self.matches.len())
            .unwrap_or(true)
        {
            self.current = Some(0);
        }
    }

    /// Close the bar and drop the highlights it painted.
    fn close(&mut self) {
        self.open = false;
        self.matches.clear();
        self.current = None;
        self.last_revision = 0;
    }

    fn step(&mut self, doc: &mut Document, forward: bool) {
        if self.matches.is_empty() {
            return;
        }
        let from = doc
            .selection
            .map(|(a, b)| if forward { b } else { a })
            .unwrap_or(0);
        if let Some(index) = next_match(&self.matches, from, forward, self.options.wrap) {
            self.current = Some(index);
            let range = self.matches[index].clone();
            let line = doc.line_col(range.start).0;
            doc.requested_selection = Some((range.start, range.end));
            doc.selection = Some((range.start, range.end));
            doc.scroll_to_line = Some(line);
        }
    }

    fn replace_current(&mut self, doc: &mut Document) -> bool {
        let Some(index) = self.current else {
            return false;
        };
        let Some(range) = self.matches.get(index).cloned() else {
            return false;
        };
        let replacement = match self.finder.replace_match(
            &doc.text[range.clone()],
            &self.query,
            &self.options,
            &self.replacement,
        ) {
            Ok(text) => text,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        let before = doc.text.clone();
        let cursor_before = range.start;
        doc.text.replace_range(range.clone(), &replacement);
        doc.edited();
        let cursor = (range.start + replacement.len()).min(doc.text.len());
        doc.record_change(&before, cursor_before, cursor);
        doc.requested_selection = Some((cursor, cursor));
        self.last_revision = 0;
        self.refresh(doc);
        self.current = next_match(&self.matches, cursor, true, true);
        true
    }

    fn replace_all(&mut self, doc: &mut Document) -> usize {
        if self.query.is_empty() {
            return 0;
        }
        let replacement = self.replacement.clone();
        match self
            .finder
            .replace_all(&doc.text, &self.query, &self.options, &replacement)
        {
            Ok(text) => {
                let count = self.matches.len();
                let before = doc.text.clone();
                doc.set_text(text);
                doc.record_change(&before, 0, 0);
                self.last_revision = 0;
                self.refresh(doc);
                self.current = None;
                self.error = None;
                count
            }
            Err(error) => {
                self.error = Some(error);
                0
            }
        }
    }
}

struct SearchState {
    query: String,
    options: SearchOptions,
    results: Vec<FileHit>,
    message: Option<String>,
    receiver: Option<Receiver<Vec<FileHit>>>,
    searched: bool,
}

impl SearchState {
    fn new() -> Self {
        Self {
            query: String::new(),
            options: SearchOptions::default(),
            results: Vec::new(),
            message: None,
            receiver: None,
            searched: false,
        }
    }
}

pub struct NotepadApp {
    docs: Vec<Document>,
    active: usize,
    next_doc_id: u64,
    untitled_counter: usize,
    settings: Settings,
    system_accent: Option<[u8; 3]>,
    theme: Theme,
    theme_signature: Option<(bool, [u8; 3], f32, f32)>,
    find: FindState,
    search: SearchState,
    side: Side,
    side_open: bool,
    zen_mode: bool,
    preview_open: bool,
    diff_pair: Option<(usize, usize)>,
    applied_topmost: Option<bool>,
    dragging_tab: Option<usize>,
    tree: Option<FileTree>,
    expanded: std::collections::HashSet<PathBuf>,
    dialog: Option<Dialog>,
    menu: Option<MenuKind>,
    menu_tab: usize,
    menu_pos: Pos2,
    menu_rect: Rect,
    anchor_rect: Rect,
    snackbar: Option<Snackbar>,
    highlight: HighlightCache,
    last_session_save: Instant,
    last_auto_save: Instant,
    last_poll: Instant,
    /// Set when the session on disk is out of date; cleared by persist_session.
    session_dirty: bool,
    close_all_pending: bool,
    window_title: String,
    /// Document that last received keyboard focus; a change re-focuses the editor.
    focused_doc: Option<u64>,
}

impl NotepadApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings, files: Vec<PathBuf>) -> Self {
        let system_accent = session::read_windows_accent_color();
        let seed_color = settings.effective_seed_color(system_accent);
        let theme = Theme::new(
            settings.dark_mode,
            seed_color,
            settings.ui_scale,
            settings.editor_font_size,
        );
        theme.apply(&cc.egui_ctx);

        let mut app = Self {
            docs: Vec::new(),
            active: 0,
            next_doc_id: 1,
            untitled_counter: 0,
            theme_signature: Some((
                settings.dark_mode,
                [seed_color.r(), seed_color.g(), seed_color.b()],
                settings.ui_scale,
                settings.editor_font_size,
            )),
            system_accent,
            settings,
            theme,
            find: FindState::new(),
            search: SearchState::new(),
            side: Side::Files,
            side_open: false,
            zen_mode: false,
            preview_open: false,
            diff_pair: None,
            applied_topmost: None,
            dragging_tab: None,
            tree: None,
            expanded: std::collections::HashSet::new(),
            dialog: None,
            menu: None,
            menu_tab: 0,
            menu_pos: Pos2::ZERO,
            menu_rect: Rect::NOTHING,
            anchor_rect: Rect::NOTHING,
            snackbar: None,
            highlight: HighlightCache::default(),
            last_session_save: Instant::now(),
            last_auto_save: Instant::now(),
            last_poll: Instant::now(),
            session_dirty: true,
            close_all_pending: false,
            window_title: String::new(),
            focused_doc: None,
        };
        if files.is_empty() {
            app.restore_session();
        } else {
            app.open_startup_files(files);
        }
        app
    }

    /// Open files or folders passed on the command line instead of the previous session.
    fn open_startup_files(&mut self, files: Vec<PathBuf>) {
        for path in files {
            if path.is_dir() {
                self.settings.last_folder = Some(path.to_string_lossy().into_owned());
                self.expanded.clear();
                self.expanded.insert(path.clone());
                self.tree = Some(FileTree::new(path));
                self.side = Side::Files;
                self.side_open = true;
            } else {
                self.open_path(path);
            }
        }
        if self.docs.is_empty() {
            self.new_document();
        }
    }

    fn restore_session(&mut self) {
        if !self.settings.restore_session {
            self.new_document();
            return;
        }
        let saved = session::load_session();
        for file in saved.files {
            let mut restored = None;
            if let Some(path) = &file.path {
                if let Ok(mut doc) = Document::from_path(self.next_doc_id, Path::new(path)) {
                    if let Some(backup) = &file.backup {
                        if let Some(text) = session::read_backup(backup) {
                            doc.set_text(text);
                            doc.saved_revision = 0;
                            doc.backup_name = Some(backup.clone());
                        }
                    }
                    restored = Some(doc);
                }
            } else if let Some(backup) = &file.backup {
                if let Some(text) = session::read_backup(backup) {
                    let mut doc = Document::untitled(self.next_doc_id, self.untitled_counter + 1);
                    self.untitled_counter += 1;
                    doc.set_text(text);
                    doc.saved_revision = 0;
                    doc.backup_name = Some(backup.clone());
                    restored = Some(doc);
                }
            }
            if let Some(mut doc) = restored {
                self.next_doc_id += 1;
                doc.encoding = file.encoding;
                doc.eol = file.eol;
                doc.pinned = file.pinned;
                let byte = doc.byte_of(file.line.max(1), file.column.max(1));
                doc.requested_selection = Some((byte, byte));
                doc.selection = Some((byte, byte));
                doc.update_cursor(byte, None);
                doc.scroll_to_line = Some(file.line.max(1));
                self.docs.push(doc);
            }
        }
        if self.docs.is_empty() {
            self.new_document();
        } else {
            self.active = saved.active.min(self.docs.len().saturating_sub(1));
        }
        if let Some(folder) = &self.settings.last_folder {
            let path = PathBuf::from(folder);
            if path.is_dir() {
                self.tree = Some(FileTree::new(path));
                self.expanded.insert(PathBuf::from(folder));
            }
        }
    }

    fn active_doc(&self) -> Option<&Document> {
        self.docs.get(self.active)
    }

    fn active_doc_mut(&mut self) -> Option<&mut Document> {
        self.docs.get_mut(self.active)
    }

    fn new_document(&mut self) {
        self.untitled_counter += 1;
        let doc = Document::untitled(self.next_doc_id, self.untitled_counter);
        self.next_doc_id += 1;
        self.docs.push(doc);
        self.active = self.docs.len() - 1;
        self.dialog = None;
        self.session_dirty = true;
    }

    fn open_path(&mut self, path: PathBuf) {
        if let Some(index) = self
            .docs
            .iter()
            .position(|d| d.path.as_deref() == Some(path.as_path()))
        {
            self.active = index;
            return;
        }
        match Document::from_path(self.next_doc_id, &path) {
            Ok(mut doc) => {
                self.next_doc_id += 1;
                doc.requested_selection = Some((0, 0));
                self.docs.push(doc);
                self.active = self.docs.len() - 1;
                self.settings.push_recent(&path);
                self.settings.last_folder = path
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .or_else(|| self.settings.last_folder.clone());
                self.session_dirty = true;
            }
            Err(error) => {
                self.dialog = Some(Dialog::Error {
                    title: "Cannot open file".to_owned(),
                    message: format!("{}\n\n{error}", path.display()),
                });
            }
        }
    }

    fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter(
                "Text and code",
                &[
                    "txt", "md", "rs", "c", "h", "cpp", "hpp", "cs", "java", "py", "js", "ts",
                    "json", "toml", "yaml", "yml", "xml", "html", "css", "scss", "ini", "cfg",
                    "log", "sh", "bat", "ps1", "sql", "lua", "rb", "go", "php", "swift", "kt",
                ],
            )
            .add_filter("All files", &["*"]);
        if let Some(folder) = &self.settings.last_folder {
            dialog = dialog.set_directory(folder);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(path);
        }
    }

    fn open_folder_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        if let Some(folder) = &self.settings.last_folder {
            dialog = dialog.set_directory(folder);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.settings.last_folder = Some(folder.to_string_lossy().into_owned());
            self.tree = Some(FileTree::new(folder.clone()));
            self.expanded.clear();
            self.expanded.insert(folder);
            self.side = Side::Files;
            self.side_open = true;
        }
    }

    fn save_document(&mut self, index: usize) -> bool {
        let needs_path = self
            .docs
            .get(index)
            .map(|d| d.path.is_none())
            .unwrap_or(false);
        if needs_path {
            return self.save_document_as(index);
        }
        let result = self.docs[index].save();
        match result {
            Ok(()) => {
                let path = self.docs[index].path.clone();
                if let Some(path) = path {
                    self.settings.push_recent(&path);
                }
                session::remove_backup(&format!("{}.txt", self.docs[index].id));
                if let Some(old_backup) = self.docs[index].backup_name.take() {
                    session::remove_backup(&old_backup);
                }
                let name = self.docs[index].title.clone();
                self.snackbar = Some(Snackbar {
                    text: format!("Saved {name}"),
                    created: Instant::now(),
                });
                true
            }
            Err(error) => {
                self.dialog = Some(Dialog::Error {
                    title: "Cannot save file".to_owned(),
                    message: error.to_string(),
                });
                false
            }
        }
    }

    fn save_document_as(&mut self, index: usize) -> bool {
        let mut dialog = rfd::FileDialog::new();
        if let Some(path) = &self.docs[index].path {
            if let Some(parent) = path.parent() {
                dialog = dialog.set_directory(parent);
            }
            dialog = dialog.set_file_name(&self.docs[index].title);
        } else if let Some(folder) = &self.settings.last_folder {
            dialog = dialog.set_directory(folder);
        }
        let Some(path) = dialog.save_file() else {
            return false;
        };
        match self.docs[index].save_as(&path) {
            Ok(()) => {
                self.settings.push_recent(&path);
                session::remove_backup(&format!("{}.txt", self.docs[index].id));
                if let Some(old_backup) = self.docs[index].backup_name.take() {
                    session::remove_backup(&old_backup);
                }
                self.snackbar = Some(Snackbar {
                    text: format!("Saved {}", self.docs[index].title),
                    created: Instant::now(),
                });
                true
            }
            Err(error) => {
                self.dialog = Some(Dialog::Error {
                    title: "Cannot save file".to_owned(),
                    message: error.to_string(),
                });
                false
            }
        }
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.docs.len() {
            return;
        }
        if self.docs[index].is_dirty() {
            self.dialog = Some(Dialog::ConfirmClose { index });
            return;
        }
        self.remove_doc(index);
    }

    fn remove_doc(&mut self, index: usize) {
        if index >= self.docs.len() {
            return;
        }
        let mut doc = self.docs.remove(index);
        let id = doc.id;
        self.highlight.forget(id);
        session::remove_backup(&format!("{id}.txt"));
        if let Some(old_backup) = doc.backup_name.take() {
            session::remove_backup(&old_backup);
        }
        self.session_dirty = true;
        if self.docs.is_empty() {
            self.new_document();
            return;
        }
        if self.active >= self.docs.len() {
            self.active = self.docs.len() - 1;
        } else if index < self.active {
            self.active -= 1;
        }
    }

    fn close_all(&mut self) {
        let dirty: Vec<usize> = self
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.is_dirty())
            .map(|(i, _)| i)
            .collect();
        if dirty.is_empty() {
            self.close_all_pending = false;
            for mut doc in self.docs.drain(..) {
                self.highlight.forget(doc.id);
                session::remove_backup(&format!("{}.txt", doc.id));
                if let Some(old_backup) = doc.backup_name.take() {
                    session::remove_backup(&old_backup);
                }
            }
            self.new_document();
        } else {
            self.close_all_pending = true;
            self.dialog = Some(Dialog::ConfirmClose { index: dirty[0] });
        }
    }

    fn ensure_theme(&mut self, ctx: &egui::Context) {
        let seed_color = self.settings.effective_seed_color(self.system_accent);
        let seed_rgb = [seed_color.r(), seed_color.g(), seed_color.b()];
        let signature = (
            self.settings.dark_mode,
            seed_rgb,
            self.settings.ui_scale,
            self.settings.editor_font_size,
        );
        if self.theme_signature != Some(signature) {
            self.theme = Theme::new(
                self.settings.dark_mode,
                seed_color,
                self.settings.ui_scale,
                self.settings.editor_font_size,
            );
            self.theme.apply(ctx);
            self.theme_signature = Some(signature);
        }
        if self.applied_topmost != Some(self.settings.always_on_top) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                if self.settings.always_on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                },
            ));
            self.applied_topmost = Some(self.settings.always_on_top);
        }
    }

    fn editor_prefs(&self) -> EditorPrefs {
        EditorPrefs {
            tab_width: self.settings.tab_width,
            use_spaces: self.settings.spaces_for_tabs,
            word_wrap: self.settings.word_wrap,
            line_numbers: self.settings.line_numbers,
        }
    }

    fn persist_session(&mut self) {
        let mut files = Vec::new();
        for doc in &self.docs {
            let backup = if doc.is_dirty() || doc.path.is_none() {
                session::write_backup(doc.id, &doc.text)
            } else {
                None
            };
            files.push(SessionFile {
                path: doc.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                backup,
                line: doc.cursor.line,
                column: doc.cursor.column,
                encoding: doc.encoding,
                eol: doc.eol,
                pinned: doc.pinned,
            });
        }
        let session = Session {
            files,
            active: self.active,
            side_panel: self.side_open,
        };
        let _ = session::save_session(&session);
        let _ = session::save_settings(&self.settings);
        self.session_dirty = false;
    }

    fn tick(&mut self, ctx: &egui::Context) {
        if self.last_poll.elapsed() >= Duration::from_secs(2) {
            for doc in &mut self.docs {
                if doc.path.is_some() {
                    doc.poll_external_change();
                }
            }
            self.last_poll = Instant::now();
        }
        if self.settings.auto_save
            && (self.last_auto_save.elapsed() >= Duration::from_secs(5)
                || !ctx.input(|i| i.focused))
        {
            for doc in &mut self.docs {
                if doc.is_dirty() && doc.path.is_some() && !doc.external_change {
                    if doc.save().is_ok() {
                        session::remove_backup(&format!("{}.txt", doc.id));
                        if let Some(old) = doc.backup_name.take() {
                            session::remove_backup(&old);
                        }
                    }
                }
            }
            self.last_auto_save = Instant::now();
        }
        if self.session_dirty && self.last_session_save.elapsed() > Duration::from_secs(20) {
            self.persist_session();
            self.last_session_save = Instant::now();
        }
        let title = match self.active_doc() {
            Some(doc) => format!(
                "{}{} - Material Notepad",
                if doc.is_dirty() { "*" } else { "" },
                doc.title
            ),
            None => "Material Notepad".to_owned(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            for file in dropped {
                if let Some(path) = file.path {
                    self.open_path(path);
                }
            }
        }
    }

    /// Ctrl + mouse wheel zooms the editor text. The wheel events are consumed so
    /// the editor does not also scroll while zooming.
    fn handle_zoom(&mut self, ctx: &egui::Context) {
        let mut steps = 0.0_f32;
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                } = event
                {
                    if modifiers.ctrl || modifiers.command {
                        steps += match unit {
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * 10.0,
                            egui::MouseWheelUnit::Point => delta.y / 50.0,
                        };
                    }
                }
            }
        });
        if steps.abs() < 0.001 {
            return;
        }
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                !matches!(
                    event,
                    egui::Event::MouseWheel { modifiers, .. }
                        if modifiers.ctrl || modifiers.command
                )
            });
        });

        let current = self.settings.editor_font_size;
        // About 10 % per wheel notch, snapped to half points.
        let factor = (steps * 0.1).exp();
        let next = ((current * factor).clamp(9.0, 32.0) * 2.0).round() / 2.0;
        if (next - current).abs() > 0.01 {
            self.settings.editor_font_size = next;
            self.snackbar = Some(Snackbar {
                text: format!("Font size {next:.1} pt"),
                created: Instant::now(),
            });
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            if ctx.input(|input| pressed(input, Key::Escape)) {
                self.dialog = None;
                self.close_all_pending = false;
            }
            return;
        }
        let mut action: Option<Action> = None;
        let mut modifiers = Modifiers::NONE;
        ctx.input(|input| {
            let m = input.modifiers;
            modifiers = m;
            if m.ctrl && m.shift {
                if pressed(input, Key::S) {
                    action = Some(Action::SaveAs);
                } else if pressed(input, Key::W) {
                    action = Some(Action::CloseAll);
                } else if pressed(input, Key::K) {
                    action = Some(Action::DeleteLine);
                } else if pressed(input, Key::Tab) {
                    action = Some(Action::PrevTab);
                } else if pressed(input, Key::F) {
                    action = Some(Action::FindInFiles);
                } else if pressed(input, Key::P) {
                    action = Some(Action::CommandPalette);
                } else if pressed(input, Key::L) {
                    action = Some(Action::SelectAllOccurrences);
                } else if pressed(input, Key::Z) {
                    action = Some(Action::Redo);
                }
            } else if m.ctrl && m.alt {
                if pressed(input, Key::ArrowUp) {
                    action = Some(Action::AddCursorAbove);
                } else if pressed(input, Key::ArrowDown) {
                    action = Some(Action::AddCursorBelow);
                }
            } else if m.ctrl {
                if pressed(input, Key::N) {
                    action = Some(Action::New);
                } else if pressed(input, Key::O) {
                    action = Some(Action::Open);
                } else if pressed(input, Key::S) {
                    action = Some(Action::Save);
                } else if pressed(input, Key::W) {
                    action = Some(Action::CloseTab);
                } else if pressed(input, Key::Z) {
                    action = Some(Action::Undo);
                } else if pressed(input, Key::Y) {
                    action = Some(Action::Redo);
                } else if pressed(input, Key::F) {
                    action = Some(Action::Find);
                } else if pressed(input, Key::H) {
                    action = Some(Action::Replace);
                } else if pressed(input, Key::G) {
                    action = Some(Action::GoToLine);
                } else if pressed(input, Key::D) {
                    action = Some(Action::DuplicateLine);
                } else if pressed(input, Key::Slash) {
                    action = Some(Action::ToggleComment);
                } else if pressed(input, Key::Tab) {
                    action = Some(Action::NextTab);
                } else if pressed(input, Key::Plus) || pressed(input, Key::Equals) {
                    action = Some(Action::ZoomIn);
                } else if pressed(input, Key::Minus) {
                    action = Some(Action::ZoomOut);
                } else if pressed(input, Key::Num0) {
                    action = Some(Action::ZoomReset);
                } else if pressed(input, Key::Comma) {
                    action = Some(Action::Settings);
                }
            } else if m.alt {
                if pressed(input, Key::Z) {
                    action = Some(Action::ToggleWrap);
                } else if pressed(input, Key::ArrowUp) {
                    action = Some(Action::MoveLineUp);
                } else if pressed(input, Key::ArrowDown) {
                    action = Some(Action::MoveLineDown);
                }
            } else if pressed(input, Key::F3) {
                action = Some(if m.shift {
                    Action::FindPrev
                } else {
                    Action::FindNext
                });
            } else if pressed(input, Key::F1) {
                action = Some(Action::Shortcuts);
            } else if pressed(input, Key::F10) {
                action = Some(Action::Menu);
            } else if pressed(input, Key::F11) {
                action = Some(Action::ToggleZenMode);
            } else if pressed(input, Key::Escape) {
                if self.dialog.is_some() {
                    self.dialog = None;
                } else if self.menu.is_some() {
                    self.menu = None;
                } else if self.find.open {
                    self.find.close();
                } else if self.diff_pair.is_some() {
                    self.diff_pair = None;
                } else if self.zen_mode {
                    self.zen_mode = false;
                }
            }
        });
        if let Some(action) = action {
            self.do_action(action, ctx);
        }
        // Consume the keys that opened actions so TextEdit does not also see them.
        if modifiers.ctrl {
            for key in [
                Key::N,
                Key::O,
                Key::S,
                Key::W,
                Key::Z,
                Key::Y,
                Key::F,
                Key::H,
                Key::G,
                Key::D,
                Key::P,
                Key::L,
                Key::Slash,
            ] {
                ctx.input_mut(|i| {
                    i.consume_key(Modifiers::CTRL, key);
                    i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, key);
                });
            }
        }
    }

    fn do_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::New => self.new_document(),
            Action::Open => self.open_dialog(),
            Action::OpenFolder => self.open_folder_dialog(),
            Action::Save => {
                let index = self.active;
                self.save_document(index);
            }
            Action::SaveAs => {
                let index = self.active;
                self.save_document_as(index);
            }
            Action::Reload => {
                if let Some(doc) = self.active_doc_mut() {
                    if let Err(error) = doc.reload() {
                        self.dialog = Some(Dialog::Error {
                            title: "Cannot reload file".to_owned(),
                            message: error.to_string(),
                        });
                    } else {
                        self.find.last_revision = 0;
                    }
                }
            }
            Action::CloseTab => {
                let index = self.active;
                self.close_tab(index);
            }
            Action::CloseAll => self.close_all(),
            Action::Exit => {
                self.persist_session();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Action::Undo => {
                if let Some(doc) = self.active_doc_mut() {
                    if doc.undo() {
                        self.find.last_revision = 0;
                        self.session_dirty = true;
                    }
                }
            }
            Action::Redo => {
                if let Some(doc) = self.active_doc_mut() {
                    if doc.redo() {
                        self.find.last_revision = 0;
                        self.session_dirty = true;
                    }
                }
            }
            Action::DuplicateLine => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::duplicate_line(doc);
                }
            }
            Action::DeleteLine => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::delete_line(doc);
                }
            }
            Action::ToggleComment => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::toggle_comment(doc);
                }
            }
            Action::MoveLineUp => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::move_line(doc, true);
                }
            }
            Action::MoveLineDown => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::move_line(doc, false);
                }
            }
            Action::SelectAll => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::select_all(doc);
                }
                let id = self.active_doc().map(|d| Id::new(("editor", d.id)));
                if let Some(id) = id {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            }
            Action::SelectAllOccurrences => {
                if let Some(doc) = self.active_doc_mut() {
                    let count = editor::select_all_occurrences(doc);
                    if count > 1 {
                        self.snackbar = Some(Snackbar {
                            text: format!("{count} cursors active (Esc to exit)"),
                            created: Instant::now(),
                        });
                    }
                }
            }
            Action::AddCursorAbove => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::add_cursor_vertical(doc, false);
                }
            }
            Action::AddCursorBelow => {
                if let Some(doc) = self.active_doc_mut() {
                    editor::add_cursor_vertical(doc, true);
                }
            }
            Action::Find => {
                self.find.open = true;
                self.find.replace = false;
                self.find.focus_query = true;
                self.find.last_revision = 0;
            }
            Action::Replace => {
                self.find.open = true;
                self.find.replace = true;
                self.find.focus_query = true;
                self.find.last_revision = 0;
            }
            Action::FindNext => {
                self.find.open = true;
                let active = self.active;
                if let Some(doc) = self.docs.get_mut(active) {
                    self.find.refresh(doc);
                    self.find.step(doc, true);
                }
            }
            Action::FindPrev => {
                self.find.open = true;
                let active = self.active;
                if let Some(doc) = self.docs.get_mut(active) {
                    self.find.refresh(doc);
                    self.find.step(doc, false);
                }
            }
            Action::GoToLine => {
                let value = self
                    .active_doc()
                    .map(|d| d.cursor.line.to_string())
                    .unwrap_or_default();
                self.dialog = Some(Dialog::GoToLine { value });
            }
            Action::FindInFiles => {
                self.side = Side::Search;
                self.side_open = true;
            }
            Action::ToggleWrap => {
                self.settings.word_wrap = !self.settings.word_wrap;
                self.snackbar = Some(Snackbar {
                    text: format!(
                        "Word wrap {}",
                        if self.settings.word_wrap { "on" } else { "off" }
                    ),
                    created: Instant::now(),
                });
            }
            Action::ToggleLineNumbers => {
                self.settings.line_numbers = !self.settings.line_numbers;
            }
            Action::ToggleZenMode => {
                self.zen_mode = !self.zen_mode;
                self.snackbar = Some(Snackbar {
                    text: if self.zen_mode {
                        "Zen mode on (F11 or Esc to exit)".to_owned()
                    } else {
                        "Zen mode off".to_owned()
                    },
                    created: Instant::now(),
                });
            }
            Action::ToggleAlwaysOnTop => {
                self.settings.always_on_top = !self.settings.always_on_top;
                self.snackbar = Some(Snackbar {
                    text: if self.settings.always_on_top {
                        "Always on top enabled".to_owned()
                    } else {
                        "Always on top disabled".to_owned()
                    },
                    created: Instant::now(),
                });
            }
            Action::TogglePreview => {
                self.preview_open = !self.preview_open;
            }
            Action::CompareTabs => {
                if self.docs.len() < 2 {
                    self.snackbar = Some(Snackbar {
                        text: "Open at least two tabs to compare".to_owned(),
                        created: Instant::now(),
                    });
                } else {
                    let other = if self.active == 0 { 1 } else { 0 };
                    self.dialog = Some(Dialog::PickDiff { other });
                }
            }
            Action::TogglePinTab => {
                let idx = self.active;
                self.toggle_pin_tab(idx);
            }
            Action::ShowStats => {
                self.dialog = Some(Dialog::Stats);
            }
            Action::CommandPalette => {
                self.dialog = Some(Dialog::CommandPalette {
                    query: String::new(),
                    selected: 0,
                    focus: true,
                });
            }
            Action::SortLinesAsc => self.apply_transform(|s| transform::sort_lines(s, false)),
            Action::SortLinesDesc => self.apply_transform(|s| transform::sort_lines(s, true)),
            Action::RemoveDuplicateLines => self.apply_transform(transform::remove_duplicate_lines),
            Action::RemoveEmptyLines => self.apply_transform(transform::remove_empty_lines),
            Action::TrimTrailingSpaces => self.apply_transform(transform::trim_trailing_whitespace),
            Action::Uppercase => self.apply_transform(|s| s.to_uppercase()),
            Action::Lowercase => self.apply_transform(|s| s.to_lowercase()),
            Action::TitleCase => self.apply_transform(transform::to_title_case),
            Action::ToggleSnakeCamel => self.apply_transform(transform::toggle_snake_camel),
            Action::FormatJson => self.apply_json_transform(true),
            Action::MinifyJson => self.apply_json_transform(false),
            Action::ZoomIn => {
                self.settings.editor_font_size = (self.settings.editor_font_size + 1.0).min(32.0);
            }
            Action::ZoomOut => {
                self.settings.editor_font_size = (self.settings.editor_font_size - 1.0).max(9.0);
            }
            Action::ZoomReset => self.settings.editor_font_size = 14.0,
            Action::Settings => {
                self.dialog = Some(Dialog::Settings {
                    draft: self.settings.clone(),
                })
            }
            Action::About => self.dialog = Some(Dialog::About),
            Action::Shortcuts => self.dialog = Some(Dialog::Shortcuts),
            Action::Menu => {
                let anchor = Rect::from_min_size(Pos2::new(6.0, 40.0), Vec2::splat(28.0));
                self.anchor_rect = anchor;
                self.menu_pos = anchor.left_bottom();
                self.menu = Some(MenuKind::Main);
            }
            Action::NextTab => {
                if !self.docs.is_empty() {
                    self.active = (self.active + 1) % self.docs.len();
                }
            }
            Action::PrevTab => {
                if !self.docs.is_empty() {
                    self.active = (self.active + self.docs.len() - 1) % self.docs.len();
                }
            }
        }
    }

    fn apply_transform(&mut self, func: impl FnOnce(&str) -> String) {
        let Some(doc) = self.active_doc_mut() else {
            return;
        };
        let before = doc.text.clone();
        let cursor_before = doc
            .selection
            .map(|(a, b)| a.min(b))
            .unwrap_or(0)
            .min(doc.text.len());
        if let Some((a, b)) = doc.selection.filter(|(a, b)| b > a) {
            let a = doc.snap(a);
            let b = doc.snap(b).max(a);
            let replaced = func(&doc.text[a..b]);
            doc.text.replace_range(a..b, &replaced);
            doc.edited();
            let end = a + replaced.len();
            doc.record_change(&before, cursor_before, end);
            doc.requested_selection = Some((a, end));
        } else {
            let replaced = func(&doc.text);
            doc.set_text(replaced);
            doc.record_change(&before, cursor_before, 0);
            doc.requested_selection = Some((0, 0));
        }
        self.find.last_revision = 0;
        self.session_dirty = true;
    }

    fn apply_json_transform(&mut self, pretty: bool) {
        let tab_width = self.settings.tab_width;
        let Some(doc) = self.active_doc_mut() else {
            return;
        };
        let result = if pretty {
            transform::format_json(&doc.text, tab_width)
        } else {
            transform::minify_json(&doc.text)
        };
        match result {
            Ok(formatted) => {
                let before = doc.text.clone();
                doc.set_text(formatted);
                doc.record_change(&before, 0, 0);
                doc.requested_selection = Some((0, 0));
                self.find.last_revision = 0;
                self.session_dirty = true;
                self.snackbar = Some(Snackbar {
                    text: if pretty {
                        "JSON formatted".to_owned()
                    } else {
                        "JSON minified".to_owned()
                    },
                    created: Instant::now(),
                });
            }
            Err(err) => {
                self.dialog = Some(Dialog::Error {
                    title: "Invalid JSON".to_owned(),
                    message: err,
                });
            }
        }
    }

    fn toggle_pin_tab(&mut self, index: usize) {
        let Some(doc) = self.docs.get_mut(index) else {
            return;
        };
        doc.pinned = !doc.pinned;
        let target_id = doc.id;
        let active_id = self.docs.get(self.active).map(|d| d.id);
        // Keep pinned tabs grouped on the left while preserving relative order.
        let mut pinned: Vec<Document> = Vec::new();
        let mut unpinned: Vec<Document> = Vec::new();
        for d in self.docs.drain(..) {
            if d.pinned {
                pinned.push(d);
            } else {
                unpinned.push(d);
            }
        }
        self.docs.extend(pinned);
        self.docs.extend(unpinned);
        if let Some(aid) = active_id {
            if let Some(pos) = self.docs.iter().position(|d| d.id == aid) {
                self.active = pos;
            }
        } else if let Some(pos) = self.docs.iter().position(|d| d.id == target_id) {
            self.active = pos;
        }
        self.session_dirty = true;
    }

    fn close_others(&mut self, keep_index: usize) {
        let Some(keep_id) = self.docs.get(keep_index).map(|d| d.id) else {
            return;
        };
        let dirty_other = self
            .docs
            .iter()
            .enumerate()
            .find(|(_, d)| d.id != keep_id && !d.pinned && d.is_dirty())
            .map(|(i, _)| i);
        if let Some(idx) = dirty_other {
            self.dialog = Some(Dialog::ConfirmClose { index: idx });
            return;
        }
        let mut kept = Vec::new();
        for mut doc in self.docs.drain(..) {
            if doc.id == keep_id || doc.pinned {
                kept.push(doc);
            } else {
                self.highlight.forget(doc.id);
                session::remove_backup(&format!("{}.txt", doc.id));
                if let Some(old) = doc.backup_name.take() {
                    session::remove_backup(&old);
                }
            }
        }
        self.docs = kept;
        self.active = self
            .docs
            .iter()
            .position(|d| d.id == keep_id)
            .unwrap_or(0);
        self.session_dirty = true;
    }

    // ---------------------------------------------------------------- chrome

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let full = ui.available_rect_before_wrap();
        let height = full.height();
        let show_preview_btn = self
            .active_doc()
            .map(|d| matches!(d.language, Language::Markdown | Language::Csv))
            .unwrap_or(false)
            || self.preview_open;
        let right_count = if show_preview_btn { 5.0 } else { 4.0 };
        let right_width = right_count * 28.0 + 12.0;
        let left_width = 30.0;
        let tabs_width = (full.width() - left_width - right_width).max(80.0);

        let mut left = ui.child_ui(
            Rect::from_min_size(full.min, Vec2::new(left_width, height)),
            Layout::left_to_right(Align::Center),
        );
        let menu_button =
            widgets::icon_button_small(&mut left, &theme, crate::icons::MENU, "Menu", false);
        if menu_button.clicked() {
            self.open_menu(MenuKind::Main, menu_button.rect, ui);
        }

        let tabs_origin = Pos2::new(full.min.x + left_width, full.min.y);
        let mut tabs = ui.child_ui(
            Rect::from_min_size(tabs_origin, Vec2::new(tabs_width, height)),
            Layout::left_to_right(Align::Center),
        );
        let mut activate: Option<usize> = None;
        let mut close: Option<usize> = None;
        let mut context_menu: Option<(usize, Rect)> = None;
        let mut swap_tabs: Option<(usize, usize)> = None;
        let mut add = false;
        let mut tab_rects: Vec<Rect> = Vec::with_capacity(self.docs.len());
        ScrollArea::horizontal()
            .id_source("tabs")
            .auto_shrink([false, true])
            .show(&mut tabs, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (index, doc) in self.docs.iter().enumerate() {
                        let tab = widgets::tab_compact(
                            ui,
                            &theme,
                            &doc.title,
                            doc.is_dirty(),
                            index == self.active,
                            doc.pinned,
                            true,
                        );
                        tab_rects.push(tab.response.rect);
                        if tab.response.drag_started_by(egui::PointerButton::Primary) {
                            self.dragging_tab = Some(index);
                        }
                        if tab.response.clicked() && !tab.close_clicked {
                            activate = Some(index);
                        }
                        if tab.response.clicked_by(egui::PointerButton::Middle) && !doc.pinned {
                            close = Some(index);
                        }
                        if tab.response.secondary_clicked() {
                            context_menu = Some((index, tab.response.rect));
                        }
                        if tab.close_clicked {
                            close = Some(index);
                        }
                    }
                    if widgets::icon_button_small(ui, &theme, crate::icons::ADD, "New tab", false)
                        .clicked()
                    {
                        add = true;
                    }
                });
            });

        if let Some(drag_idx) = self.dragging_tab {
            if ui.input(|i| i.pointer.primary_down()) {
                if let Some(pos) = ui.ctx().pointer_interact_pos() {
                    if let Some((target_idx, _)) = tab_rects
                        .iter()
                        .enumerate()
                        .find(|(idx, r)| *idx != drag_idx && r.contains(pos))
                    {
                        if self.docs.get(drag_idx).map(|d| d.pinned)
                            == self.docs.get(target_idx).map(|d| d.pinned)
                        {
                            swap_tabs = Some((drag_idx, target_idx));
                        }
                    }
                }
            } else {
                self.dragging_tab = None;
            }
        }

        let mut right = ui.child_ui(
            Rect::from_min_max(Pos2::new(tabs_origin.x + tabs_width, full.min.y), full.max),
            Layout::right_to_left(Align::Center),
        );
        right.spacing_mut().item_spacing.x = 2.0;
        if widgets::icon_button_small(
            &mut right,
            &theme,
            crate::icons::SETTINGS,
            "Preferences",
            false,
        )
        .clicked()
        {
            self.dialog = Some(Dialog::Settings {
                draft: self.settings.clone(),
            });
        }
        let theme_icon = if self.settings.dark_mode {
            crate::icons::LIGHT_MODE
        } else {
            crate::icons::DARK_MODE
        };
        if widgets::icon_button_small(&mut right, &theme, theme_icon, "Toggle theme", false)
            .clicked()
        {
            self.settings.dark_mode = !self.settings.dark_mode;
        }
        if widgets::icon_button_small(
            &mut right,
            &theme,
            crate::icons::PIN,
            "Always on top",
            self.settings.always_on_top,
        )
        .clicked()
        {
            self.settings.always_on_top = !self.settings.always_on_top;
        }
        if show_preview_btn
            && widgets::icon_button_small(
                &mut right,
                &theme,
                crate::icons::VISIBILITY,
                "Toggle preview",
                self.preview_open,
            )
            .clicked()
        {
            self.preview_open = !self.preview_open;
        }
        if widgets::icon_button_small(&mut right, &theme, crate::icons::SEARCH, "Find", false)
            .clicked()
        {
            self.find.open = true;
            self.find.focus_query = true;
        }

        if let Some((a, b)) = swap_tabs {
            self.docs.swap(a, b);
            if self.active == a {
                self.active = b;
            } else if self.active == b {
                self.active = a;
            }
            self.dragging_tab = Some(b);
            self.session_dirty = true;
        }
        if let Some(index) = activate {
            self.active = index;
        }
        if let Some(index) = close {
            self.close_tab(index);
        }
        if let Some((index, anchor)) = context_menu {
            self.menu_tab = index;
            self.open_menu(MenuKind::Tab, anchor, ui);
        }
        if add {
            self.new_document();
        }
    }

    fn open_menu(&mut self, kind: MenuKind, anchor: Rect, ui: &egui::Ui) {
        self.menu = Some(kind);
        self.anchor_rect = anchor;
        self.menu_pos = anchor.left_bottom();
        ui.ctx().request_repaint();
    }

    // ------------------------------------------------------------ side panels

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        match self.side {
            Side::Files => self.files_panel(ui),
            Side::Search => self.search_panel(ui),
        }
    }

    fn files_panel(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let s = theme.scheme;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Explorer")
                    .font(theme.type_scale.label_large.clone())
                    .color(s.on_surface_variant),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button_small(ui, &theme, crate::icons::CLOSE, "Hide panel", false)
                    .clicked()
                {
                    self.side_open = false;
                }
                if widgets::icon_button_small(
                    ui,
                    &theme,
                    crate::icons::FOLDER_OPEN,
                    "Open folder",
                    false,
                )
                .clicked()
                {
                    self.open_folder_dialog();
                }
                if widgets::icon_button_small(ui, &theme, crate::icons::REFRESH, "Refresh", false)
                    .clicked()
                {
                    if let Some(tree) = &mut self.tree {
                        tree.refresh();
                    }
                }
            });
        });
        ui.add_space(2.0);
        widgets::divider(ui, &theme);

        ScrollArea::vertical()
            .id_source("files-tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(root) = self.tree.as_ref().map(|t| t.root.clone()) {
                    ui.label(
                        RichText::new(self.tree.as_ref().map(|t| t.name()).unwrap_or_default())
                            .font(theme.type_scale.label_medium.clone())
                            .color(s.primary),
                    );
                    self.tree_dir(ui, &root, 0);
                } else {
                    widgets::body_text(
                        ui,
                        &theme,
                        "Open a folder to browse its files. Files open in tabs and keep their own language mode.",
                    );
                    ui.add_space(8.0);
                    if widgets::tonal_button(ui, &theme, "Open folder", Some(crate::icons::FOLDER_OPEN)).clicked() {
                        self.open_folder_dialog();
                    }
                }
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Recent")
                        .font(theme.type_scale.label_medium.clone())
                        .color(s.primary),
                );
                let recent = self.settings.recent.clone();
                if recent.is_empty() {
                    widgets::caption(ui, &theme, "No recent files yet", s.outline);
                }
                for path in recent {
                    let name = Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    let response = file_row(ui, &theme, &name, &path);
                    if response.clicked() {
                        self.open_path(PathBuf::from(&path));
                    }
                }
            });
    }

    fn tree_dir(&mut self, ui: &mut egui::Ui, dir: &Path, depth: usize) {
        if depth > 8 {
            return;
        }
        let theme = self.theme.clone();
        let entries = match self.tree.as_mut() {
            Some(tree) => tree.entries(dir).to_vec(),
            None => return,
        };
        for entry in entries {
            let expanded = self.expanded.contains(&entry.path);
            if entry.is_dir {
                let response = dir_row(ui, &theme, &entry.name, expanded, depth);
                if response.clicked() {
                    if expanded {
                        self.expanded.remove(&entry.path);
                    } else {
                        self.expanded.insert(entry.path.clone());
                    }
                }
                if expanded {
                    self.tree_dir(ui, &entry.path, depth + 1);
                }
            } else {
                let response = file_row_indented(ui, &theme, &entry.name, depth);
                if response.clicked() {
                    self.open_path(entry.path.clone());
                }
            }
        }
    }

    fn search_panel(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let s = theme.scheme;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Find in files")
                    .font(theme.type_scale.label_large.clone())
                    .color(s.on_surface_variant),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button_small(ui, &theme, crate::icons::CLOSE, "Hide panel", false)
                    .clicked()
                {
                    self.side_open = false;
                }
            });
        });
        ui.add_space(2.0);
        let width = ui.available_width() - 4.0;
        widgets::text_field(
            ui,
            &theme,
            Id::new("search-files-query"),
            &mut self.search.query,
            "Search text",
            width,
        );
        ui.horizontal(|ui| {
            if widgets::chip(ui, &theme, "Aa", self.search.options.case_sensitive).clicked() {
                self.search.options.case_sensitive = !self.search.options.case_sensitive;
            }
            if widgets::chip(ui, &theme, "W", self.search.options.whole_word).clicked() {
                self.search.options.whole_word = !self.search.options.whole_word;
            }
            if widgets::chip(ui, &theme, ".*", self.search.options.regex).clicked() {
                self.search.options.regex = !self.search.options.regex;
            }
        });
        let folder = self
            .tree
            .as_ref()
            .map(|t| t.root.clone())
            .or_else(|| self.settings.last_folder.as_ref().map(PathBuf::from));
        ui.horizontal(|ui| {
            let label = folder
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "No folder selected".to_owned());
            widgets::caption(ui, &theme, &truncate(&label, 30), s.outline);
        });
        let mut start = false;
        ui.horizontal(|ui| {
            if widgets::filled_button(ui, &theme, "Search", Some(crate::icons::SEARCH)).clicked() {
                start = true;
            }
            if widgets::text_button(ui, &theme, "Choose folder").clicked() {
                self.open_folder_dialog();
            }
        });
        if start {
            if let (Some(root), false) = (folder.clone(), self.search.query.trim().is_empty()) {
                let query = self.search.query.clone();
                let options = self.search.options.clone();
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let hits = find_in_files(&root, &query, &options, 500).unwrap_or_default();
                    let _ = tx.send(hits);
                });
                self.search.receiver = Some(rx);
                self.search.message = Some("Searching...".to_owned());
            }
        }
        if let Some(receiver) = &self.search.receiver {
            if let Ok(hits) = receiver.try_recv() {
                self.search.message = Some(format!("{} matches", hits.len()));
                self.search.results = hits;
                self.search.receiver = None;
                self.search.searched = true;
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
        }
        ui.add_space(4.0);
        if let Some(message) = &self.search.message {
            widgets::caption(ui, &theme, message, s.on_surface_variant);
        }
        widgets::divider(ui, &theme);
        ScrollArea::vertical()
            .id_source("search-results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.search.searched && self.search.results.is_empty() {
                    widgets::body_text(ui, &theme, "No matches found.");
                }
                let mut open: Option<(PathBuf, usize, usize)> = None;
                for hit in &self.search.results {
                    let name = hit
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
                    if response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            widgets::RADIUS_SMALL,
                            s.state_layer(s.primary, 18),
                        );
                    }
                    ui.painter().text(
                        rect.left_top() + Vec2::new(6.0, 4.0),
                        Align2::LEFT_TOP,
                        format!("{name}:{}", hit.line),
                        theme.type_scale.label_medium.clone(),
                        s.on_surface,
                    );
                    ui.painter().text(
                        rect.left_top() + Vec2::new(6.0, 22.0),
                        Align2::LEFT_TOP,
                        truncate(hit.line_text.trim_start(), 44),
                        theme.type_scale.label_small.clone(),
                        s.on_surface_variant,
                    );
                    if response.clicked() {
                        open = Some((hit.path.clone(), hit.line, hit.column));
                    }
                }
                if let Some((path, line, column)) = open {
                    self.open_path(path);
                    if let Some(doc) = self.active_doc_mut() {
                        let byte = doc.byte_of(line, column);
                        doc.requested_selection = Some((byte, byte));
                        doc.scroll_to_line = Some(line);
                    }
                }
            });
    }

    // ---------------------------------------------------------------- editor

    fn editor_area(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let s = theme.scheme;
        if self.docs.is_empty() {
            self.empty_state(ui);
            return;
        }
        if self.active >= self.docs.len() {
            self.active = self.docs.len() - 1;
        }
        let doc_id = self.docs[self.active].id;
        // Focus the editor of the document shown on screen, so typing (and Enter)
        // works straight after launch, opening a file or switching tabs.
        if self.focused_doc != Some(doc_id) {
            let id = Id::new(("editor", doc_id));
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
            self.focused_doc = Some(doc_id);
        }

        // External change banner.
        if self.docs[self.active].external_change {
            Frame::none()
                .fill(s.tertiary_container)
                .rounding(widgets::RADIUS_MEDIUM)
                .inner_margin(Margin::symmetric(14.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        widgets::icon(ui, crate::icons::WARNING, 18.0, s.on_tertiary_container);
                        ui.label(
                            RichText::new("This file changed on disk.")
                                .font(theme.type_scale.label_medium.clone())
                                .color(s.on_tertiary_container),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::text_button(ui, &theme, "Ignore").clicked() {
                                self.docs[self.active].external_change = false;
                            }
                            if widgets::text_button(ui, &theme, "Reload").clicked() {
                                let _ = self.docs[self.active].reload();
                                self.find.last_revision = 0;
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }

        let prefs = self.editor_prefs();
        let active = self.active;
        self.find.refresh(&self.docs[active]);

        if let Some((a, b)) = self.diff_pair {
            if let (Some(left), Some(right)) = (self.docs.get(a), self.docs.get(b)) {
                let left_title = left.title.clone();
                let left_text = left.text.clone();
                let right_title = right.title.clone();
                let right_text = right.text.clone();
                let mut close_diff = false;
                Frame::none()
                    .fill(s.surface_container_lowest)
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if widgets::outlined_button(ui, &theme, "Close diff").clicked() {
                                close_diff = true;
                            }
                        });
                        ui.add_space(6.0);
                        preview::show_diff_view(
                            ui,
                            &theme,
                            &left_title,
                            &left_text,
                            &right_title,
                            &right_text,
                        );
                    });
                if close_diff {
                    self.diff_pair = None;
                }
                return;
            } else {
                self.diff_pair = None;
            }
        }

        let show_preview = self.preview_open
            && matches!(self.docs[active].language, Language::Markdown | Language::Csv);

        if show_preview {
            let total_w = ui.available_width();
            let half_w = (total_w - 6.0).max(200.0) * 0.5;
            let height = ui.available_height() - 4.0;
            ui.horizontal(|ui| {
                ui.allocate_ui(Vec2::new(half_w, height), |ui| {
                    Frame::none()
                        .fill(s.surface_container_lowest)
                        .inner_margin(Margin::same(0.0))
                        .show(ui, |ui| {
                            ui.set_min_height(height);
                            let current_match = self.find.current;
                            let doc = &mut self.docs[active];
                            let event = editor::show(
                                ui,
                                &self.theme,
                                doc,
                                &prefs,
                                &self.find.matches,
                                current_match,
                                &mut self.highlight,
                            );
                            if event.changed {
                                self.find.last_revision = 0;
                                self.session_dirty = true;
                            }
                        });
                });
                ui.allocate_ui(Vec2::new(half_w, height), |ui| {
                    ui.set_min_height(height);
                    let lang = self.docs[active].language;
                    let text = &self.docs[active].text;
                    match lang {
                        Language::Markdown => preview::show_markdown_preview(ui, &theme, text),
                        Language::Csv => preview::show_csv_preview(ui, &theme, text),
                        _ => {}
                    }
                });
            });
        } else {
            Frame::none()
                .fill(s.surface_container_lowest)
                .inner_margin(Margin::same(0.0))
                .show(ui, |ui| {
                    ui.set_min_height(ui.available_height() - 4.0);
                    let current_match = self.find.current;
                    let (docs, find, highlight, theme, prefs) = (
                        &mut self.docs,
                        &mut self.find,
                        &mut self.highlight,
                        &self.theme,
                        &prefs,
                    );
                    let doc = &mut docs[active];
                    let event = editor::show(
                        ui,
                        theme,
                        doc,
                        prefs,
                        &find.matches,
                        current_match,
                        highlight,
                    );
                    if event.changed {
                        find.last_revision = 0;
                        self.session_dirty = true;
                    }
                });
        }

        // Floating find bar.
        if self.find.open && self.docs.get(active).is_some() {
            self.find_bar(ui);
        }
    }

    fn find_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let s = theme.scheme;
        let editor_rect = ui.max_rect();
        let width = 520.0_f32.min(editor_rect.width() - 48.0).max(340.0);
        let pos = Pos2::new(editor_rect.right() - width - 24.0, editor_rect.top() + 12.0);
        let query_id = Id::new("find-query");
        let mut close = false;
        let mut step: Option<bool> = None;
        let mut replace_one = false;
        let mut replace_everything = false;
        let mut enter = false;

        egui::Area::new(Id::new("find-bar"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                Frame::none()
                    .fill(s.surface_container_high)
                    .rounding(widgets::RADIUS_LARGE)
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 6.0),
                        blur: 22.0,
                        spread: 0.0,
                        color: s.shadow_color(self.theme.dark),
                    })
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_width(width - 28.0);
                        ui.horizontal(|ui| {
                            let field_width = width - 160.0;
                            let response = widgets::text_field(
                                ui,
                                &theme,
                                query_id,
                                &mut self.find.query,
                                "Find",
                                field_width,
                            );
                            if self.find.focus_query {
                                response.request_focus();
                                self.find.focus_query = false;
                            }
                            if response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                enter = true;
                            }
                            let count = match (self.find.matches.len(), self.find.current) {
                                (0, _) => "0/0".to_owned(),
                                (total, Some(current)) => format!("{}/{}", current + 1, total),
                                (total, None) => format!("0/{total}"),
                            };
                            widgets::caption(ui, &theme, &count, s.on_surface_variant);
                            if widgets::icon_button(
                                ui,
                                &theme,
                                crate::icons::ARROW_UP,
                                "Previous (Shift+F3)",
                                false,
                            )
                            .clicked()
                            {
                                step = Some(false);
                            }
                            if widgets::icon_button(
                                ui,
                                &theme,
                                crate::icons::ARROW_DOWN,
                                "Next (F3)",
                                false,
                            )
                            .clicked()
                            {
                                step = Some(true);
                            }
                            if widgets::icon_button(ui, &theme, crate::icons::CLOSE, "Close", false)
                                .clicked()
                            {
                                close = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if widgets::chip(ui, &theme, "Aa", self.find.options.case_sensitive)
                                .clicked()
                            {
                                self.find.options.case_sensitive =
                                    !self.find.options.case_sensitive;
                                self.find.last_revision = 0;
                            }
                            if widgets::chip(ui, &theme, "Word", self.find.options.whole_word)
                                .clicked()
                            {
                                self.find.options.whole_word = !self.find.options.whole_word;
                                self.find.last_revision = 0;
                            }
                            if widgets::chip(ui, &theme, "Regex", self.find.options.regex).clicked()
                            {
                                self.find.options.regex = !self.find.options.regex;
                                self.find.last_revision = 0;
                            }
                            if widgets::chip(ui, &theme, "Wrap", self.find.options.wrap).clicked() {
                                self.find.options.wrap = !self.find.options.wrap;
                                self.find.last_revision = 0;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::text_button(
                                    ui,
                                    &theme,
                                    if self.find.replace {
                                        "Hide replace"
                                    } else {
                                        "Replace"
                                    },
                                )
                                .clicked()
                                {
                                    self.find.replace = !self.find.replace;
                                }
                            });
                        });
                        if let Some(error) = &self.find.error {
                            ui.label(
                                RichText::new(format!("Invalid pattern: {error}"))
                                    .font(theme.type_scale.label_small.clone())
                                    .color(s.error),
                            );
                        }
                        if self.find.replace {
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                widgets::text_field(
                                    ui,
                                    &theme,
                                    Id::new("replace-field"),
                                    &mut self.find.replacement,
                                    "Replace with",
                                    width - 210.0,
                                );
                                if widgets::tonal_button(ui, &theme, "Replace", None).clicked() {
                                    replace_one = true;
                                }
                                if widgets::filled_button(ui, &theme, "All", None).clicked() {
                                    replace_everything = true;
                                }
                            });
                        }
                    });
            });

        if close {
            self.find.close();
        }
        if let Some(forward) = step {
            let active = self.active;
            if let Some(doc) = self.docs.get_mut(active) {
                self.find.refresh(doc);
                self.find.step(doc, forward);
            }
        }
        if enter {
            let active = self.active;
            if let Some(doc) = self.docs.get_mut(active) {
                self.find.refresh(doc);
                self.find.step(doc, true);
            }
        }
        if replace_one {
            let active = self.active;
            if let Some(doc) = self.docs.get_mut(active) {
                if self.find.replace_current(doc) {
                    self.session_dirty = true;
                }
            }
        }
        if replace_everything {
            let active = self.active;
            if let Some(doc) = self.docs.get_mut(active) {
                let count = self.find.replace_all(doc);
                if count > 0 {
                    self.snackbar = Some(Snackbar {
                        text: format!(
                            "Replaced {count} occurrence{}",
                            if count == 1 { "" } else { "s" }
                        ),
                        created: Instant::now(),
                    });
                    self.session_dirty = true;
                }
            }
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let s = theme.scheme;
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.18);
            widgets::logo(ui, &theme, 84.0);
            ui.add_space(16.0);
            ui.label(
                RichText::new("Nothing open")
                    .font(theme.type_scale.headline_small.clone())
                    .color(s.on_surface),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Create a document or open a file to start editing.")
                    .font(theme.type_scale.body_medium.clone())
                    .color(s.on_surface_variant),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if widgets::filled_button(ui, &theme, "New file", Some(crate::icons::ADD)).clicked()
                {
                    self.new_document();
                }
                if widgets::tonal_button(ui, &theme, "Open file", Some(crate::icons::FOLDER_OPEN))
                    .clicked()
                {
                    self.open_dialog();
                }
            });
            if !self.settings.recent.is_empty() {
                ui.add_space(28.0);
                ui.label(
                    RichText::new("Recent")
                        .font(theme.type_scale.label_large.clone())
                        .color(s.primary),
                );
                for path in self.settings.recent.clone().into_iter().take(6) {
                    let name = Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    if file_row(ui, &theme, &name, &path).clicked() {
                        self.open_path(PathBuf::from(path));
                    }
                }
            }
        });
    }

    // ------------------------------------------------------------- status bar

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        let s = theme.scheme;
        let Some(doc) = self.active_doc() else {
            return;
        };
        let cursor: CursorInfo = doc.cursor;
        let lines = doc.line_count();
        let words = doc.text.split_whitespace().count();
        let bytes = doc.text.len();
        let multi = doc.multi_cursors.len();
        let encoding = doc.encoding;
        let eol = doc.eol;
        let language = doc.language;

        // Painted by hand so the text sits exactly in the middle of the strip:
        // the panel's layout would otherwise place the row against its bottom.
        let rect = ui
            .max_rect()
            .intersect(ui.clip_rect())
            .intersect(ui.ctx().screen_rect());
        let center_y = rect.center().y;
        let font = theme.type_scale.label_small.clone();
        let painter = ui.painter();

        let mut x = rect.left() + 10.0;
        let put = |x: &mut f32, text: &str, color: Color32| {
            if text.is_empty() {
                return;
            }
            let galley =
                ui.fonts(|fonts| fonts.layout_no_wrap(text.to_owned(), font.clone(), color));
            let size = galley.size();
            painter.galley(Pos2::new(*x, center_y - size.y / 2.0), galley, color);
            *x += size.x;
        };

        put(
            &mut x,
            &format!("Ln {}, Col {}", cursor.line, cursor.column),
            s.on_surface_variant,
        );
        if multi > 1 {
            x += 12.0;
            put(&mut x, &format!("{multi} cursors"), s.primary);
        } else if cursor.selection_chars > 0 {
            x += 12.0;
            put(
                &mut x,
                &format!("Sel {}", cursor.selection_chars),
                s.primary,
            );
        }
        x += 12.0;
        let stats_start = x;
        put(
            &mut x,
            &format!("{lines} lines · {words} words · {}", transform::format_bytes(bytes)),
            s.outline,
        );
        let stats_rect = Rect::from_min_max(
            Pos2::new(stats_start - 4.0, rect.top() + 1.0),
            Pos2::new(x + 4.0, rect.bottom() - 1.0),
        );
        if ui
            .interact(stats_rect, Id::new("status-stats"), Sense::click())
            .on_hover_text("Document statistics")
            .clicked()
        {
            self.dialog = Some(Dialog::Stats);
        }

        // Right side: encoding, line endings and language, each opening a menu.
        // Labels are laid out once; the same galley is measured and painted.
        let item_labels: [(&str, MenuKind); 3] = [
            (encoding.label(), MenuKind::Encoding),
            (eol.label(), MenuKind::Eol),
            (language.name(), MenuKind::Language),
        ];
        let galleys: Vec<std::sync::Arc<egui::Galley>> = item_labels
            .iter()
            .map(|(label, _)| {
                ui.fonts(|fonts| {
                    fonts.layout_no_wrap((*label).to_owned(), font.clone(), s.on_surface_variant)
                })
            })
            .collect();
        let gap = 14.0;
        let pad_x = 6.0;
        let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + pad_x * 2.0).collect();
        let mut right = rect.right() - 8.0;
        let mut clicks: Vec<(MenuKind, Rect)> = Vec::new();
        for (index, (_, kind)) in item_labels.iter().enumerate().rev() {
            let width = widths[index];
            let item_rect = Rect::from_min_max(
                Pos2::new(right - width, rect.top() + 1.0),
                Pos2::new(right, rect.bottom() - 1.0),
            );
            right -= width + gap;
            let response = ui.interact(item_rect, Id::new(("status-item", index)), Sense::click());
            if response.hovered() {
                painter.rect_filled(
                    item_rect,
                    widgets::RADIUS_SMALL,
                    s.state_layer(s.on_surface, 24),
                );
            }
            let color = s.on_surface_variant;
            let galley = galleys[index].clone();
            painter.galley(
                Pos2::new(
                    item_rect.center().x - galley.size().x / 2.0,
                    center_y - galley.size().y / 2.0,
                ),
                galley,
                color,
            );
            let _ = color;
            if response.clicked() {
                clicks.push((*kind, item_rect));
            }
        }
        for (kind, anchor) in clicks {
            self.open_menu(kind, anchor, ui);
        }
    }

    // ----------------------------------------------------------------- menus

    fn menus(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.menu else {
            return;
        };
        let theme = self.theme.clone();
        let s = theme.scheme;
        let screen = ctx.screen_rect();
        let mut chosen: Option<Action> = None;
        let mut chosen_encoding: Option<Encoding> = None;
        let mut chosen_eol: Option<Eol> = None;
        let mut chosen_language: Option<Language> = None;
        let mut chosen_recent: Option<PathBuf> = None;
        let mut close = false;
        let width = if kind == MenuKind::Language {
            260.0
        } else {
            300.0
        };
        // End the menu on a row boundary so a scrolled menu never shows half a row.
        let available = (screen.height() - 120.0).max(240.0);
        let rows = ((available - 16.0) / 38.0).floor().max(4.0);
        let max_height = rows * 38.0 + 12.0;
        let x = self.menu_pos.x.min(screen.right() - width - 8.0).max(8.0);
        let position = if self.menu_pos.y > screen.bottom() - 60.0 {
            Pos2::new(x, self.menu_pos.y - 8.0)
        } else {
            Pos2::new(
                x,
                self.menu_pos
                    .y
                    .min(screen.bottom() - max_height - 8.0)
                    .max(8.0),
            )
        };
        let pivot = if self.menu_pos.y > screen.bottom() - 60.0 {
            Align2::LEFT_BOTTOM
        } else {
            Align2::LEFT_TOP
        };
        let area = egui::Area::new(Id::new("app-menu"))
            .order(Order::Foreground)
            .fixed_pos(position)
            .pivot(pivot)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(s.surface_container)
                    .rounding(widgets::RADIUS_MEDIUM + 4.0)
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 8.0),
                        blur: 26.0,
                        spread: 0.0,
                        color: s.shadow_color(self.theme.dark),
                    })
                    .inner_margin(Margin::same(6.0))
                    .show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        match kind {
                            MenuKind::Main => {
                                let recent = self.settings.recent.clone();
                                egui::ScrollArea::vertical()
                                    .id_source("main-menu-scroll")
                                    .max_height(max_height - 12.0)
                                    .show(ui, |ui| {
                                        menu_section(ui, &theme, "File");
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::NEW_FILE),
                                            "New",
                                            Some("Ctrl+N"),
                                        ) {
                                            chosen = Some(Action::New);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::FOLDER_OPEN),
                                            "Open...",
                                            Some("Ctrl+O"),
                                        ) {
                                            chosen = Some(Action::Open);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::FOLDER),
                                            "Open folder...",
                                            None,
                                        ) {
                                            chosen = Some(Action::OpenFolder);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::SAVE),
                                            "Save",
                                            Some("Ctrl+S"),
                                        ) {
                                            chosen = Some(Action::Save);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::SAVE_AS),
                                            "Save as...",
                                            Some("Ctrl+Shift+S"),
                                        ) {
                                            chosen = Some(Action::SaveAs);
                                        }
                                        menu_divider(ui, &theme);
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::RESTART),
                                            "Reload from disk",
                                            None,
                                        ) {
                                            chosen = Some(Action::Reload);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::CLOSE),
                                            "Close tab",
                                            Some("Ctrl+W"),
                                        ) {
                                            chosen = Some(Action::CloseTab);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::CLEAR_ALL),
                                            "Close all",
                                            Some("Ctrl+Shift+W"),
                                        ) {
                                            chosen = Some(Action::CloseAll);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::TERMINAL),
                                            "Exit",
                                            None,
                                        ) {
                                            chosen = Some(Action::Exit);
                                        }
                                        if !recent.is_empty() {
                                            menu_divider(ui, &theme);
                                            menu_section(ui, &theme, "Recent");
                                            for path in recent.into_iter().take(4) {
                                                let name = Path::new(&path)
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().into_owned())
                                                    .unwrap_or(path.clone());
                                                if menu_row(
                                                    ui,
                                                    &theme,
                                                    Some(crate::icons::HISTORY),
                                                    &name,
                                                    None,
                                                ) {
                                                    chosen_recent = Some(PathBuf::from(path));
                                                }
                                            }
                                        }
                                        menu_divider(ui, &theme);
                                        menu_section(ui, &theme, "Edit");
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::UNDO),
                                            "Undo",
                                            Some("Ctrl+Z"),
                                        ) {
                                            chosen = Some(Action::Undo);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::REDO),
                                            "Redo",
                                            Some("Ctrl+Y"),
                                        ) {
                                            chosen = Some(Action::Redo);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::COPY),
                                            "Duplicate line",
                                            Some("Ctrl+D"),
                                        ) {
                                            chosen = Some(Action::DuplicateLine);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::DELETE),
                                            "Delete line",
                                            Some("Ctrl+Shift+K"),
                                        ) {
                                            chosen = Some(Action::DeleteLine);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::COMMENT),
                                            "Toggle comment",
                                            Some("Ctrl+/"),
                                        ) {
                                            chosen = Some(Action::ToggleComment);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::SELECT_ALL),
                                            "Select all",
                                            Some("Ctrl+A"),
                                        ) {
                                            chosen = Some(Action::SelectAll);
                                        }
                                        menu_divider(ui, &theme);
                                        menu_section(ui, &theme, "Search");
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::SEARCH),
                                            "Find...",
                                            Some("Ctrl+F"),
                                        ) {
                                            chosen = Some(Action::Find);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::FIND_REPLACE),
                                            "Replace...",
                                            Some("Ctrl+H"),
                                        ) {
                                            chosen = Some(Action::Replace);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::PIN),
                                            "Go to line...",
                                            Some("Ctrl+G"),
                                        ) {
                                            chosen = Some(Action::GoToLine);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::SEARCH),
                                            "Find in files",
                                            Some("Ctrl+Shift+F"),
                                        ) {
                                            chosen = Some(Action::FindInFiles);
                                        }
                                        menu_divider(ui, &theme);
                                        menu_section(ui, &theme, "Transform & Tools");
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::TERMINAL),
                                            "Command palette...",
                                            Some("Ctrl+Shift+P"),
                                        ) {
                                            chosen = Some(Action::CommandPalette);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::CODE),
                                            "Format JSON",
                                            None,
                                        ) {
                                            chosen = Some(Action::FormatJson);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::TEXT_FIELDS),
                                            "Sort lines (A-Z)",
                                            None,
                                        ) {
                                            chosen = Some(Action::SortLinesAsc);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::DELETE_SWEEP),
                                            "Remove duplicate lines",
                                            None,
                                        ) {
                                            chosen = Some(Action::RemoveDuplicateLines);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::VISIBILITY),
                                            "Compare tabs...",
                                            None,
                                        ) {
                                            chosen = Some(Action::CompareTabs);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::INFO),
                                            "Document statistics...",
                                            None,
                                        ) {
                                            chosen = Some(Action::ShowStats);
                                        }
                                        menu_divider(ui, &theme);
                                        menu_section(ui, &theme, "View");
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::FULLSCREEN),
                                            "Zen mode",
                                            Some("F11"),
                                        ) {
                                            chosen = Some(Action::ToggleZenMode);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::PIN),
                                            "Always on top",
                                            None,
                                        ) {
                                            chosen = Some(Action::ToggleAlwaysOnTop);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::WRAP_TEXT),
                                            "Word wrap",
                                            Some("Alt+Z"),
                                        ) {
                                            chosen = Some(Action::ToggleWrap);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::LIST_NUMBERED),
                                            "Line numbers",
                                            None,
                                        ) {
                                            chosen = Some(Action::ToggleLineNumbers);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::ZOOM_IN),
                                            "Zoom in",
                                            Some("Ctrl++"),
                                        ) {
                                            chosen = Some(Action::ZoomIn);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::ZOOM_OUT),
                                            "Zoom out",
                                            Some("Ctrl+-"),
                                        ) {
                                            chosen = Some(Action::ZoomOut);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::RESTART),
                                            "Reset zoom",
                                            Some("Ctrl+0"),
                                        ) {
                                            chosen = Some(Action::ZoomReset);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::PALETTE),
                                            "Preferences...",
                                            None,
                                        ) {
                                            chosen = Some(Action::Settings);
                                        }
                                        menu_divider(ui, &theme);
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::KEYBOARD),
                                            "Keyboard shortcuts",
                                            Some("F1"),
                                        ) {
                                            chosen = Some(Action::Shortcuts);
                                        }
                                        if menu_row(
                                            ui,
                                            &theme,
                                            Some(crate::icons::INFO),
                                            "About",
                                            None,
                                        ) {
                                            chosen = Some(Action::About);
                                        }
                                    });
                            }
                            MenuKind::Encoding => {
                                menu_section(ui, &theme, "Encoding");
                                for encoding in Encoding::all() {
                                    let selected =
                                        self.active_doc().map(|d| d.encoding) == Some(encoding);
                                    if menu_row_checked(ui, &theme, encoding.label(), selected) {
                                        chosen_encoding = Some(encoding);
                                    }
                                }
                            }
                            MenuKind::Eol => {
                                menu_section(ui, &theme, "Line endings");
                                for eol in Eol::all() {
                                    let selected = self.active_doc().map(|d| d.eol) == Some(eol);
                                    if menu_row_checked(ui, &theme, eol.label(), selected) {
                                        chosen_eol = Some(eol);
                                    }
                                }
                            }
                            MenuKind::Language => {
                                menu_section(ui, &theme, "Language");
                                ScrollArea::vertical()
                                    .id_source("language-menu")
                                    .max_height(360.0)
                                    .show(ui, |ui| {
                                        for language in Language::ALL {
                                            let selected = self.active_doc().map(|d| d.language)
                                                == Some(language);
                                            if menu_row_checked(
                                                ui,
                                                &theme,
                                                language.name(),
                                                selected,
                                            ) {
                                                chosen_language = Some(language);
                                            }
                                        }
                                    });
                            }
                            MenuKind::Tab => {
                                let idx = self.menu_tab;
                                let pinned = self.docs.get(idx).map(|d| d.pinned).unwrap_or(false);
                                menu_section(ui, &theme, "Tab");
                                if menu_row(
                                    ui,
                                    &theme,
                                    Some(crate::icons::PIN),
                                    if pinned { "Unpin tab" } else { "Pin tab" },
                                    None,
                                ) {
                                    self.menu = None;
                                    self.toggle_pin_tab(idx);
                                    return;
                                }
                                if idx != self.active
                                    && menu_row(
                                        ui,
                                        &theme,
                                        Some(crate::icons::VISIBILITY),
                                        "Compare with active tab",
                                        None,
                                    )
                                {
                                    self.diff_pair = Some((self.active, idx));
                                    self.menu = None;
                                    return;
                                }
                                if !pinned
                                    && menu_row(
                                        ui,
                                        &theme,
                                        Some(crate::icons::CLOSE),
                                        "Close tab",
                                        None,
                                    )
                                {
                                    self.menu = None;
                                    self.close_tab(idx);
                                    return;
                                }
                                if self.docs.len() > 1
                                    && menu_row(
                                        ui,
                                        &theme,
                                        Some(crate::icons::CLEAR_ALL),
                                        "Close other tabs",
                                        None,
                                    )
                                {
                                    self.menu = None;
                                    self.close_others(idx);
                                    return;
                                }
                            }
                        }
                    });
            });
        self.menu_rect = area.response.rect;
        let anchor = self.anchor_rect;
        if ctx.input(|i| i.pointer.any_click()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !self.menu_rect.contains(pos) && !anchor.contains(pos) {
                    close = true;
                }
            }
        }
        if close {
            self.menu = None;
        }
        if let Some(encoding) = chosen_encoding {
            if let Some(doc) = self.active_doc_mut() {
                if doc.encoding != encoding {
                    doc.encoding = encoding;
                    if encoding != Encoding::Utf8 && doc.eol == Eol::Cr {
                        doc.eol = Eol::Lf;
                    }
                    doc.edited();
                    self.session_dirty = true;
                }
            }
            self.menu = None;
        }
        if let Some(eol) = chosen_eol {
            if let Some(doc) = self.active_doc_mut() {
                if doc.eol != eol {
                    doc.eol = eol;
                    doc.edited();
                    self.session_dirty = true;
                }
            }
            self.menu = None;
        }
        if let Some(language) = chosen_language {
            if let Some(doc) = self.active_doc_mut() {
                doc.language = language;
            }
            self.menu = None;
        }
        if let Some(path) = chosen_recent {
            self.open_path(path);
            self.menu = None;
        }
        if let Some(action) = chosen {
            self.menu = None;
            self.do_action(action, ctx);
        }
    }

    // --------------------------------------------------------------- dialogs

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let theme = self.theme.clone();
        let s = theme.scheme;
        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("scrim"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(110));
            });

        let mut close = false;
        let mut confirm: Option<(usize, bool)> = None;
        match &mut dialog {
            Dialog::Settings { draft } => {
                let mut apply = false;
                centered_area(ctx, 560.0, |ui| {
                    dialog_header(ui, &theme, "Preferences", Some(crate::icons::TUNE));
                    ui.add_space(10.0);
                    settings_ui(ui, &theme, draft);
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        if widgets::outlined_button(ui, &theme, "Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::filled_button(ui, &theme, "Save preferences", None)
                                .clicked()
                            {
                                apply = true;
                            }
                        });
                    });
                });
                if apply {
                    self.settings = draft.clone();
                    self.settings.sanitize();
                    let _ = session::save_settings(&self.settings);
                    close = true;
                }
            }
            Dialog::About => {
                centered_area(ctx, 460.0, |ui| {
                    ui.vertical_centered(|ui| {
                        widgets::logo(ui, &theme, 64.0);
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Material Notepad")
                                .font(theme.type_scale.headline_small.clone())
                                .color(s.on_surface),
                        );
                        ui.label(
                            RichText::new(concat!("Version ", env!("CARGO_PKG_VERSION"), " - Rust"))
                                .font(theme.type_scale.label_large.clone())
                                .color(s.primary),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(
                                "A text editor written in Rust with a Material Design 3 interface, forked from \
                                 Notepad Next. Tabs, syntax highlighting, find and replace, find in files, \
                                 session restore and dynamic colour from a seed palette.",
                            )
                            .font(theme.type_scale.body_medium.clone())
                            .color(s.on_surface_variant),
                        );
                        ui.add_space(8.0);
                        widgets::caption(
                            ui,
                            &theme,
                            "Based on Notepad Next, (c) its contributors, GPL-3.0",
                            s.outline,
                        );
                        ui.add_space(16.0);
                        if widgets::filled_button(ui, &theme, "Close", None).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Shortcuts => {
                centered_area(ctx, 560.0, |ui| {
                    dialog_header(
                        ui,
                        &theme,
                        "Keyboard shortcuts",
                        Some(crate::icons::KEYBOARD),
                    );
                    ui.add_space(8.0);
                    let shortcuts = [
                        ("New / Open / Save", "Ctrl+N / Ctrl+O / Ctrl+S"),
                        ("Save as", "Ctrl+Shift+S"),
                        ("Close tab / all", "Ctrl+W / Ctrl+Shift+W"),
                        ("Next / previous tab", "Ctrl+Tab / Ctrl+Shift+Tab"),
                        ("Find / replace", "Ctrl+F / Ctrl+H"),
                        ("Find next / previous", "F3 / Shift+F3"),
                        ("Go to line", "Ctrl+G"),
                        ("Undo / redo", "Ctrl+Z / Ctrl+Y"),
                        ("Duplicate / delete line", "Ctrl+D / Ctrl+Shift+K"),
                        ("Move line", "Alt+Up / Alt+Down"),
                        ("Toggle comment", "Ctrl+/"),
                        ("Select all occurrences", "Ctrl+Shift+L"),
                        ("Add cursor above / below", "Ctrl+Alt+Up / Down"),
                        ("Command palette", "Ctrl+Shift+P"),
                        ("Zen mode", "F11"),
                        ("Word wrap", "Alt+Z"),
                        ("Zoom in / out / reset", "Ctrl++ / Ctrl+- / Ctrl+0"),
                        ("Find in files", "Ctrl+Shift+F"),
                        ("Preferences", "Ctrl+,"),
                    ];
                    egui::ScrollArea::vertical()
                        .id_source("shortcuts-scroll")
                        .max_height(376.0)
                        .show(ui, |ui| {
                            for (label, keys) in shortcuts {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(label)
                                            .font(theme.type_scale.body_medium.clone())
                                            .color(s.on_surface),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        widgets::caption(ui, &theme, keys, s.primary);
                                    });
                                });
                            }
                        });
                    ui.add_space(14.0);
                    if widgets::filled_button(ui, &theme, "Close", None).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::GoToLine { value } => {
                let id = Id::new("goto-line-field");
                let mut submit = false;
                centered_area(ctx, 380.0, |ui| {
                    dialog_header(ui, &theme, "Go to line", Some(crate::icons::PIN));
                    ui.add_space(10.0);
                    let response = widgets::text_field(ui, &theme, id, value, "Line number", 300.0);
                    response.request_focus();
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        submit = true;
                    }
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if widgets::outlined_button(ui, &theme, "Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::filled_button(ui, &theme, "Go", None).clicked() {
                                submit = true;
                            }
                        });
                    });
                });
                if submit {
                    if let Ok(line) = value.trim().parse::<usize>() {
                        if let Some(doc) = self.active_doc_mut() {
                            let line = line.clamp(1, doc.line_count());
                            editor::goto_line(doc, line);
                        }
                    }
                    close = true;
                }
            }
            Dialog::ConfirmClose { index } => {
                let title = self
                    .docs
                    .get(*index)
                    .map(|d| d.title.clone())
                    .unwrap_or_else(|| "document".to_owned());
                centered_area(ctx, 440.0, |ui| {
                    dialog_header(ui, &theme, "Unsaved changes", Some(crate::icons::WARNING));
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!(
                            "\"{title}\" has changes that are not saved. Save them before closing?"
                        ))
                        .font(theme.type_scale.body_medium.clone())
                        .color(s.on_surface_variant),
                    );
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        if widgets::text_button(ui, &theme, "Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::filled_button(ui, &theme, "Save", None).clicked() {
                                confirm = Some((*index, true));
                            }
                            if widgets::tonal_button(ui, &theme, "Discard", None).clicked() {
                                confirm = Some((*index, false));
                            }
                        });
                    });
                });
            }
            Dialog::Error { title, message } => {
                let title = title.clone();
                let message = message.clone();
                centered_area(ctx, 460.0, |ui| {
                    dialog_header(ui, &theme, &title, Some(crate::icons::ERROR));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(message)
                            .font(theme.type_scale.body_medium.clone())
                            .color(s.on_surface_variant),
                    );
                    ui.add_space(16.0);
                    if widgets::filled_button(ui, &theme, "OK", None).clicked() {
                        close = true;
                    }
                });
            }
            Dialog::CommandPalette {
                query,
                selected,
                focus,
            } => {
                let mut run_action: Option<Action> = None;
                let mut switch_tab: Option<usize> = None;
                let mut set_lang: Option<Language> = None;

                enum PaletteItem {
                    Act(&'static str, &'static str, Action),
                    Tab(usize, String),
                    Lang(Language),
                }

                let q = query.trim().to_lowercase();
                let mut items: Vec<PaletteItem> = Vec::new();
                for (label, shortcut, act) in palette_commands() {
                    if q.is_empty()
                        || label.to_lowercase().contains(&q)
                        || shortcut.to_lowercase().contains(&q)
                    {
                        items.push(PaletteItem::Act(label, shortcut, act));
                    }
                }
                for (i, d) in self.docs.iter().enumerate() {
                    let label = format!("Tab: {}", d.title);
                    if !q.is_empty() && label.to_lowercase().contains(&q) {
                        items.push(PaletteItem::Tab(i, label));
                    }
                }
                for lang in Language::ALL {
                    let label = format!("Language: {}", lang.name());
                    if !q.is_empty() && label.to_lowercase().contains(&q) {
                        items.push(PaletteItem::Lang(lang));
                    }
                }

                if items.is_empty() {
                    *selected = 0;
                } else {
                    *selected = (*selected).min(items.len() - 1);
                }
                if ctx.input(|i| i.key_pressed(Key::ArrowDown)) && !items.is_empty() {
                    *selected = (*selected + 1) % items.len();
                }
                if ctx.input(|i| i.key_pressed(Key::ArrowUp)) && !items.is_empty() {
                    *selected = (*selected + items.len() - 1) % items.len();
                }
                let enter = ctx.input(|i| i.key_pressed(Key::Enter));

                centered_area(ctx, 560.0, |ui| {
                    let id = Id::new("command-palette-input");
                    let resp = widgets::text_field(
                        ui,
                        &theme,
                        id,
                        query,
                        "Type a command, tab, or language...",
                        512.0,
                    );
                    if *focus {
                        resp.request_focus();
                        *focus = false;
                    }
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical()
                        .id_source("command-palette-list")
                        .max_height(320.0)
                        .show(ui, |ui| {
                            if items.is_empty() {
                                widgets::caption(ui, &theme, "No matching commands", s.outline);
                            }
                            for (idx, item) in items.iter().enumerate().take(40) {
                                let (label, shortcut) = match item {
                                    PaletteItem::Act(l, sc, _) => (*l, *sc),
                                    PaletteItem::Tab(_, l) => (l.as_str(), "Switch tab"),
                                    PaletteItem::Lang(lang) => (lang.name(), "Set language"),
                                };
                                let is_sel = idx == *selected;
                                let (rect, r) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 34.0),
                                    Sense::click(),
                                );
                                if is_sel || r.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        widgets::RADIUS_SMALL,
                                        if is_sel {
                                            s.secondary_container
                                        } else {
                                            s.state_layer(s.primary, 18)
                                        },
                                    );
                                }
                                ui.painter().text(
                                    Pos2::new(rect.left() + 10.0, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    label,
                                    theme.type_scale.body_medium.clone(),
                                    if is_sel {
                                        s.on_secondary_container
                                    } else {
                                        s.on_surface
                                    },
                                );
                                if !shortcut.is_empty() {
                                    ui.painter().text(
                                        Pos2::new(rect.right() - 10.0, rect.center().y),
                                        Align2::RIGHT_CENTER,
                                        shortcut,
                                        theme.type_scale.label_small.clone(),
                                        s.primary,
                                    );
                                }
                                let trigger = r.clicked() || (enter && is_sel);
                                if trigger {
                                    match item {
                                        PaletteItem::Act(_, _, a) => run_action = Some(*a),
                                        PaletteItem::Tab(i, _) => switch_tab = Some(*i),
                                        PaletteItem::Lang(l) => set_lang = Some(*l),
                                    }
                                    close = true;
                                }
                            }
                        });
                });

                if let Some(a) = run_action {
                    self.dialog = None;
                    self.do_action(a, ctx);
                    return;
                }
                if let Some(i) = switch_tab {
                    self.active = i;
                    self.dialog = None;
                    return;
                }
                if let Some(l) = set_lang {
                    if let Some(doc) = self.active_doc_mut() {
                        doc.language = l;
                    }
                    self.dialog = None;
                    return;
                }
            }
            Dialog::PickDiff { other } => {
                let mut start_diff = false;
                let active = self.active;
                centered_area(ctx, 440.0, |ui| {
                    dialog_header(ui, &theme, "Compare tabs", Some(crate::icons::VISIBILITY));
                    ui.add_space(8.0);
                    widgets::body_text(
                        ui,
                        &theme,
                        "Select a tab to compare with the active document:",
                    );
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_source("pick-diff-scroll")
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for (i, doc) in self.docs.iter().enumerate() {
                                if i == active {
                                    continue;
                                }
                                if menu_row_checked(ui, &theme, &doc.title, *other == i) {
                                    *other = i;
                                }
                            }
                        });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if widgets::outlined_button(ui, &theme, "Cancel").clicked() {
                            close = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::filled_button(ui, &theme, "Compare", None).clicked() {
                                start_diff = true;
                            }
                        });
                    });
                });
                if start_diff {
                    self.diff_pair = Some((active, *other));
                    close = true;
                }
            }
            Dialog::Stats => {
                let stats = self
                    .active_doc()
                    .map(|d| transform::compute_stats(&d.text))
                    .unwrap_or_default();
                let title = self
                    .active_doc()
                    .map(|d| d.title.as_str())
                    .unwrap_or("Document");
                centered_area(ctx, 380.0, |ui| {
                    dialog_header(ui, &theme, "Document statistics", Some(crate::icons::INFO));
                    ui.add_space(6.0);
                    widgets::caption(ui, &theme, title, s.primary);
                    ui.add_space(8.0);
                    for (label, value) in [
                        ("Lines", stats.lines.to_string()),
                        ("Words", stats.words.to_string()),
                        ("Characters", stats.chars.to_string()),
                        ("Characters (no spaces)", stats.chars_no_ws.to_string()),
                        ("Size", transform::format_bytes(stats.bytes)),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(label)
                                    .font(theme.type_scale.body_medium.clone())
                                    .color(s.on_surface_variant),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(value)
                                        .font(theme.type_scale.label_large.clone())
                                        .color(s.on_surface),
                                );
                            });
                        });
                    }
                    ui.add_space(14.0);
                    if widgets::filled_button(ui, &theme, "Close", None).clicked() {
                        close = true;
                    }
                });
            }
        }

        if let Some((index, save)) = confirm {
            let mut removed = false;
            if save {
                if self.save_document(index) {
                    self.remove_doc(index);
                    removed = true;
                }
            } else {
                self.remove_doc(index);
                removed = true;
            }
            self.dialog = None;
            if removed && self.close_all_pending {
                self.close_all();
            } else if !removed {
                self.close_all_pending = false;
            }
            return;
        }
        if close {
            self.dialog = None;
            self.close_all_pending = false;
        } else {
            self.dialog = Some(dialog);
        }
    }

    fn snackbar_ui(&mut self, ctx: &egui::Context) {
        let Some(snackbar) = &self.snackbar else {
            return;
        };
        if snackbar.created.elapsed() > Duration::from_secs(4) {
            self.snackbar = None;
            return;
        }
        let theme = self.theme.clone();
        let s = theme.scheme;
        egui::Area::new(Id::new("snackbar"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -56.0))
            .show(ctx, |ui| {
                Frame::none()
                    .fill(s.inverse_surface)
                    .rounding(widgets::RADIUS_SMALL + 2.0)
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 4.0),
                        blur: 18.0,
                        spread: 0.0,
                        color: s.shadow_color(self.theme.dark),
                    })
                    .inner_margin(Margin::symmetric(18.0, 12.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&snackbar.text)
                                .font(theme.type_scale.body_medium.clone())
                                .color(s.inverse_on_surface),
                        );
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

impl eframe::App for NotepadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
        self.handle_zoom(ctx);
        self.handle_shortcuts(ctx);
        self.ensure_theme(ctx);

        let s = self.theme.scheme;
        if !self.zen_mode {
            egui::TopBottomPanel::top("app-bar")
                .exact_height(40.0)
                .frame(
                    Frame::none()
                        .fill(s.surface)
                        .inner_margin(Margin::symmetric(6.0, 2.0)),
                )
                .show(ctx, |ui| self.top_bar(ui));

            egui::TopBottomPanel::bottom("status-bar")
                .exact_height(24.0)
                .frame(
                    Frame::none()
                        .fill(s.surface)
                        .inner_margin(Margin::symmetric(6.0, 1.0)),
                )
                .show(ctx, |ui| self.status_bar(ui));

            if self.side_open {
                egui::SidePanel::left("side-panel")
                    .default_width(260.0)
                    .width_range(220.0..=380.0)
                    .frame(
                        Frame::none()
                            .fill(s.surface_container_low)
                            .inner_margin(Margin::symmetric(8.0, 4.0)),
                    )
                    .show(ctx, |ui| self.side_panel(ui));
            }
        }

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(s.surface)
                    .inner_margin(Margin::same(0.0)),
            )
            .show(ctx, |ui| self.editor_area(ui));

        self.menus(ctx);
        self.dialogs(ctx);
        self.snackbar_ui(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_session();
    }
}

// ------------------------------------------------------------------ helpers

/// Physical-key aware key press check.
///
/// winit reports Ctrl+<letter> as a control character on Windows, so the
/// logical key can be lost.  Fall back to the physical key code.
fn pressed(input: &egui::InputState, key: Key) -> bool {
    if input.key_pressed(key) {
        return true;
    }
    input.events.iter().any(|event| match event {
        egui::Event::Key {
            key: logical,
            physical_key,
            pressed: true,
            ..
        } => *logical == key || physical_key == &Some(key),
        _ => false,
    })
}

fn centered_area(ctx: &egui::Context, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Area::new(Id::new("dialog-card"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let frame = Frame::none()
                .fill(ui.style().visuals.window_fill)
                .rounding(widgets::RADIUS_XL)
                .shadow(egui::epaint::Shadow {
                    offset: Vec2::new(0.0, 12.0),
                    blur: 40.0,
                    spread: 0.0,
                    color: Color32::from_black_alpha(90),
                })
                .inner_margin(Margin::same(24.0));
            frame.show(ui, |ui| {
                ui.set_width(width - 48.0);
                add(ui);
            });
        });
}

fn dialog_header(ui: &mut egui::Ui, theme: &Theme, title: &str, icon: Option<char>) {
    let s = theme.scheme;
    ui.horizontal(|ui| {
        if let Some(icon) = icon {
            widgets::icon(ui, icon, 22.0, s.primary);
        }
        ui.label(
            RichText::new(title)
                .font(theme.type_scale.title_large.clone())
                .color(s.on_surface),
        );
    });
}

fn menu_section(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    ui.add_space(4.0);
    ui.label(
        RichText::new(label)
            .font(theme.type_scale.label_small.clone())
            .color(theme.scheme.primary),
    );
}

fn menu_divider(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(2.0);
    widgets::divider(ui, theme);
    ui.add_space(2.0);
}

fn menu_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: Option<char>,
    label: &str,
    shortcut: Option<&str>,
) -> bool {
    widgets::menu_item(ui, theme, icon, label, shortcut, true).clicked()
}

fn menu_row_checked(ui: &mut egui::Ui, theme: &Theme, label: &str, selected: bool) -> bool {
    let response = widgets::menu_item(
        ui,
        theme,
        if selected {
            Some(crate::icons::CHECK)
        } else {
            None
        },
        label,
        None,
        true,
    );
    response.clicked()
}

fn settings_ui(ui: &mut egui::Ui, theme: &Theme, draft: &mut Settings) {
    let s = theme.scheme;
    ui.label(
        RichText::new("Theme")
            .font(theme.type_scale.label_medium.clone())
            .color(s.primary),
    );
    ui.horizontal(|ui| {
        if widgets::chip(ui, theme, "Light", !draft.dark_mode).clicked() {
            draft.dark_mode = false;
        }
        if widgets::chip(ui, theme, "Dark", draft.dark_mode).clicked() {
            draft.dark_mode = true;
        }
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new("Seed colour")
            .font(theme.type_scale.label_medium.clone())
            .color(s.primary),
    );
    ui.horizontal_wrapped(|ui| {
        if widgets::chip(ui, theme, "System", draft.use_system_accent).clicked() {
            draft.use_system_accent = !draft.use_system_accent;
        }
        for (name, color) in SEEDS {
            let selected = !draft.use_system_accent
                && draft.seed == [color.r(), color.g(), color.b()];
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
            ui.painter().circle_filled(rect.center(), 15.0, color);
            if selected {
                ui.painter().circle_stroke(
                    rect.center(),
                    17.0,
                    Stroke::new(2.0_f32, theme.scheme.on_surface),
                );
            }
            if response.clicked() {
                draft.use_system_accent = false;
                draft.seed = [color.r(), color.g(), color.b()];
            }
            response.on_hover_text(name);
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Editor font size")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, theme, crate::icons::ADD, "Increase", false).clicked() {
                draft.editor_font_size = (draft.editor_font_size + 1.0).min(32.0);
            }
            widgets::caption(
                ui,
                theme,
                &format!("{} pt", draft.editor_font_size as i32),
                s.on_surface,
            );
            if widgets::icon_button(ui, theme, crate::icons::REMOVE, "Decrease", false).clicked() {
                draft.editor_font_size = (draft.editor_font_size - 1.0).max(9.0);
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Tab width")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(ui, theme, crate::icons::ADD, "Increase", false).clicked() {
                draft.tab_width = (draft.tab_width + 1).min(16);
            }
            widgets::caption(ui, theme, &format!("{}", draft.tab_width), s.on_surface);
            if widgets::icon_button(ui, theme, crate::icons::REMOVE, "Decrease", false).clicked() {
                draft.tab_width = draft.tab_width.saturating_sub(1).max(1);
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Word wrap")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut value = draft.word_wrap;
            if widgets::switch(ui, theme, Id::new("pref-wrap-2"), &mut value).clicked() {
                draft.word_wrap = value;
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Line numbers")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut value = draft.line_numbers;
            if widgets::switch(ui, theme, Id::new("pref-lines"), &mut value).clicked() {
                draft.line_numbers = value;
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Restore session on start")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut value = draft.restore_session;
            if widgets::switch(ui, theme, Id::new("pref-session"), &mut value).clicked() {
                draft.restore_session = value;
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Auto-save saved files")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut value = draft.auto_save;
            if widgets::switch(ui, theme, Id::new("pref-autosave"), &mut value).clicked() {
                draft.auto_save = value;
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Always on top")
                .font(theme.type_scale.body_medium.clone())
                .color(s.on_surface),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut value = draft.always_on_top;
            if widgets::switch(ui, theme, Id::new("pref-topmost"), &mut value).clicked() {
                draft.always_on_top = value;
            }
        });
    });
}

fn palette_commands() -> [(&'static str, &'static str, Action); 39] {
    [
        ("File: New", "Ctrl+N", Action::New),
        ("File: Open...", "Ctrl+O", Action::Open),
        ("File: Open Folder...", "", Action::OpenFolder),
        ("File: Save", "Ctrl+S", Action::Save),
        ("File: Save As...", "Ctrl+Shift+S", Action::SaveAs),
        ("File: Reload from Disk", "", Action::Reload),
        ("File: Close Tab", "Ctrl+W", Action::CloseTab),
        ("File: Close All Tabs", "Ctrl+Shift+W", Action::CloseAll),
        ("Edit: Undo", "Ctrl+Z", Action::Undo),
        ("Edit: Redo", "Ctrl+Y", Action::Redo),
        ("Edit: Duplicate Line", "Ctrl+D", Action::DuplicateLine),
        ("Edit: Delete Line", "Ctrl+Shift+K", Action::DeleteLine),
        ("Edit: Toggle Comment", "Ctrl+/", Action::ToggleComment),
        ("Edit: Select All", "Ctrl+A", Action::SelectAll),
        ("Edit: Select All Occurrences", "Ctrl+Shift+L", Action::SelectAllOccurrences),
        ("Edit: Add Cursor Above", "Ctrl+Alt+Up", Action::AddCursorAbove),
        ("Edit: Add Cursor Below", "Ctrl+Alt+Down", Action::AddCursorBelow),
        ("Search: Find...", "Ctrl+F", Action::Find),
        ("Search: Replace...", "Ctrl+H", Action::Replace),
        ("Search: Go to Line...", "Ctrl+G", Action::GoToLine),
        ("Search: Find in Files", "Ctrl+Shift+F", Action::FindInFiles),
        ("Transform: Format JSON", "", Action::FormatJson),
        ("Transform: Minify JSON", "", Action::MinifyJson),
        ("Transform: Sort Lines (A-Z)", "", Action::SortLinesAsc),
        ("Transform: Sort Lines (Z-A)", "", Action::SortLinesDesc),
        ("Transform: Remove Duplicate Lines", "", Action::RemoveDuplicateLines),
        ("Transform: Remove Empty Lines", "", Action::RemoveEmptyLines),
        ("Transform: Trim Trailing Whitespace", "", Action::TrimTrailingSpaces),
        ("Transform: UPPERCASE", "", Action::Uppercase),
        ("Transform: lowercase", "", Action::Lowercase),
        ("Transform: Title Case", "", Action::TitleCase),
        ("Transform: snake_case <-> camelCase", "", Action::ToggleSnakeCamel),
        ("View: Toggle Zen Mode", "F11", Action::ToggleZenMode),
        ("View: Toggle Always on Top", "", Action::ToggleAlwaysOnTop),
        ("View: Toggle Markdown / CSV Preview", "", Action::TogglePreview),
        ("View: Compare Tabs...", "", Action::CompareTabs),
        ("View: Pin / Unpin Tab", "", Action::TogglePinTab),
        ("View: Document Statistics", "", Action::ShowStats),
        ("Preferences...", "Ctrl+,", Action::Settings),
    ]
}

fn file_row(ui: &mut egui::Ui, theme: &Theme, name: &str, path: &str) -> egui::Response {
    file_row_indented(ui, theme, name, 0).on_hover_text(path)
}

fn file_row_indented(ui: &mut egui::Ui, theme: &Theme, name: &str, depth: usize) -> egui::Response {
    let s = theme.scheme;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, widgets::RADIUS_SMALL, s.state_layer(s.primary, 18));
        }
        let icon_color = s.on_surface_variant;
        let x = rect.left() + 4.0 + depth as f32 * 14.0;
        let g = ui.fonts(|f| {
            f.layout_no_wrap(
                crate::icons::DESCRIPTION.to_string(),
                widgets::icon_font(15.0),
                icon_color,
            )
        });
        ui.painter().galley(
            Pos2::new(x, rect.center().y - g.size().y / 2.0),
            g,
            icon_color,
        );
        ui.painter().text(
            Pos2::new(x + 22.0, rect.center().y),
            Align2::LEFT_CENTER,
            truncate(name, 34),
            theme.type_scale.label_medium.clone(),
            s.on_surface,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn dir_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    name: &str,
    expanded: bool,
    depth: usize,
) -> egui::Response {
    let s = theme.scheme;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, widgets::RADIUS_SMALL, s.state_layer(s.on_surface, 16));
        }
        let x = rect.left() + 2.0 + depth as f32 * 14.0;
        let icon = if expanded {
            crate::icons::FOLDER_OPEN
        } else {
            crate::icons::FOLDER
        };
        let g =
            ui.fonts(|f| f.layout_no_wrap(icon.to_string(), widgets::icon_font(16.0), s.primary));
        ui.painter().galley(
            Pos2::new(x, rect.center().y - g.size().y / 2.0),
            g,
            s.primary,
        );
        ui.painter().text(
            Pos2::new(x + 22.0, rect.center().y),
            Align2::LEFT_CENTER,
            truncate(name, 30),
            theme.type_scale.label_medium.clone(),
            s.on_surface,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn truncate(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push_str("...");
    }
    out
}
