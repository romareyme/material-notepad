# Porting notes: Notepad Next (C++/Qt) → Material Notepad (Rust)

Material Notepad is the Rust fork; the upstream application is a Qt 6 desktop shell around Scintilla. This fork keeps the
product behaviour that matters to an editor user and rebuilds the presentation layer in Rust
on egui, with a Material Design 3 component set written from scratch.

## Unit map

| Upstream unit(s) | Rust module | Status |
| --- | --- | --- |
| `main.cpp`, `NotepadNextApplication` | `main.rs` | Reimplemented: window, fonts, icon, config bootstrap |
| `dialogs/MainWindow` (menus, docks, status bar) | `app.rs` | Reimplemented as a 40 px title bar (menu, tabs, actions), transient side panels, overflow menu and a 24 px status line. No navigation rail: the text owns the window |
| `DockedEditor`, `EditorManager`, `widgets/TabsQuickActionsBar` | `app.rs`, `editor.rs` | Tabs, dirty markers, close prompts, per-document scroll/view state |
| `ScintillaNext` (editor wrapper) | `editor.rs` | egui `TextEdit` plus gutter, current-line and find overlays; commands for line and block edits |
| `ScintillaCommenter` | `editor.rs` (`toggle_comment`) | Reimplemented with a language → comment-prefix table |
| `Finder`, `QRegexSearch`, `FindReplaceDialog` | `search.rs`, `app.rs` (`FindState`) | Literal/regex search, options, next/previous, replace one/all, inline highlights |
| `SearchResultsCollector`, `docks/SearchResultsDock` | `files.rs`, `app.rs` (`SearchState`) | Threaded find-in-files with a result list |
| `docks/FolderAsWorkspaceDock`, `docks/FileListDock` | `files.rs` (`FileTree`), `app.rs` | Expandable folder tree, refresh, recent files |
| `SessionManager` | `session.rs` | JSON session with per-document cursor, encoding, EOL and unsaved-buffer backups |
| `ApplicationSettings` | `session.rs` (`Settings`) | JSON settings with sanitising; same defaults |
| `RecentFilesListManager`, `RecentFilesListMenuBuilder` | `session.rs`, `app.rs` | MRU list capped at 12, shown in menu and empty state |
| `LanguageStylesModel`, `src/languages/*` (88 Scintilla lexers) | `highlight.rs` | 53 hand-written lexer configurations plus Markdown/markup/JSON/INI/YAML/diff parsers |
| `ScintillaSorter`, `Sorter`, `RangeAllocator`, `SelectionTracker` | – | Not needed: egui handles selection and layout |
| `MacroManager`, `MacroRecorder`, `Macro*`, `MacroEditorDialog` | – | Not ported |
| `LuaExtension`, `LuaState`, `LuaConsoleDock` | – | Not ported |
| `NppImporter` | – | Not ported |
| `Converter`, `RtfConverter`, `HtmlConverter` | – | Not ported |
| `EditorPrintPreviewRenderer` | – | Not ported |
| `ColumnEditorDialog`, `MacroRunDialog`, `MacroSaveDialog` | – | Not ported |
| `PreferencesDialog` | `app.rs` (`settings_ui`) | Ported subset: theme, seed colour, font size, tab width, wrap, line numbers, session restore |
| `widgets/QuickFindWidget` | `app.rs` (`find_bar`) | Floating Material find card |
| `widgets/EditorInfoStatusBar`, `StatusLabel` | `app.rs` (`status_bar`) | Line/column/selection/length/encoding/EOL/language/zoom |
| `widgets/FadingIndicator` | `app.rs` (`snackbar_ui`) | Snackbar feedback for saves and mode toggles |
| `DefaultDirectoryManager`, `FileDialogHelpers` | `app.rs`, `session.rs` | Native dialogs via `rfd`, last-folder memory |
| `DebugManager`, `LanguageInspectorDock`, `EditorInspectorDock`, `LuaConsoleDock` | – | Developer tools, not ported |
| `ZoomEventWatcher`, `ShiftWheelToHorizontalScrollFilter`, `ShiftMiddleClickBlocker` | – | Superseded by egui input handling and zoom shortcuts |

## Deliberate differences

1. **Editor core.** Upstream delegates everything to Scintilla (C++). The Rust fork uses
   egui's `TextEdit` for input, selection, IME, clipboard and undo, and paints the gutter,
   current line, find matches and syntax colours itself. This is a much smaller surface to
   maintain, but it does not reach Scintilla's throughput on very large files.
2. **Undo grouping.** Text typing uses egui's undo stack. Menu commands that rewrite the
   buffer (duplicate/delete/move line, toggle comment, block indent) are applied directly and
   are not merged into the same undo history. The next step is a buffer-level undo stack that
   both paths feed.
3. **Regex flavour.** Upstream uses `QRegularExpression` (PCRE). This fork uses the Rust
   `regex` crate: no look-around or backreferences, but linear-time matching.
4. **Encodings.** UTF-8, UTF-8 BOM, UTF-16 LE/BE and a Latin-1 fallback are decoded. ANSI
   code pages (notably Windows-1251) are not implemented yet; ANSI files outside Latin-1
   should be opened as UTF-8 after conversion.
5. **Highlighting budget.** Syntax highlighting is skipped above 512 KiB and egui re-lays out
   the whole buffer per frame, so multi-megabyte files are usable but not comfortable.
   Scintilla's incremental styling has no equivalent here yet.
6. **Session safety.** Unsaved buffers are written to `backups/` every 20 seconds and on exit.
   A hard crash can lose up to 20 seconds of unsaved text.
7. **Menus and chrome.** Upstream exposes a classic menu bar plus toolbars and a dock rail.
   The fork keeps one 40 px bar: a menu button, the tabs and three actions. The same commands
   live in a Material overflow menu (also on `F10`); the explorer and find-in-files panels
   are transient and close with the button in their header instead of being permanent docks.
8. **Fonts and icons.** Roboto, Roboto Mono and a 66-glyph subset of Material Symbols Rounded
   are embedded. Upstream follows the platform UI font.

## Behaviour preserved on purpose

- Document titles, dirty-state prompts and the `Untitled-N` naming scheme.
- `Ctrl+D` duplicate, `Ctrl+Shift+K` delete line, `Alt+Up/Down` move line, `Ctrl+/` comment,
  `Ctrl+G` go to line, `F3`/`Shift+F3` search navigation, `Alt+Z` word wrap.
- Session restore of open files, cursor position, encoding and line endings.
- Recent-files ordering (most recent first, duplicates moved to the front).
- Status bar fields and the ability to switch encoding, EOL and language per document.

## Suggested next ports

Each is a self-contained unit that can be verified against the C++ implementation:

1. **Macro recorder** — record `(command, argument)` steps from the existing `Action` enum,
   replay them, save/load as JSON. Upstream units: `MacroRecorder`, `Macro`, `MacroStep`.
2. **Column editor** — insert text at a column across a line range; upstream
   `ColumnEditorDialog`.
3. **Printing** — render the active buffer to a paginated PDF; upstream
   `EditorPrintPreviewRenderer`.
4. **NppImporter** — read Notepad++ `config.xml` into `Settings`/theme overrides.
5. **More lexers** — extend `highlight.rs` with the remaining Scintilla language table
   (Perl 6, Tcl, Batch variants, etc.); the spec-driven lexer makes each one small.
6. **Incremental highlighting** — cache spans per line and restyle only changed lines, which
   also removes the 512 KiB cap.

## Verification

```sh
cargo test        # 34 unit tests: palette contrast, encodings, line index, lexers, search, tree
cargo build --release
```

Manual checks used during the port: open a Rust/Markdown/JSON file and confirm token colours;
`Ctrl+F` and replace; `Ctrl+G`; tab switching and closing with unsaved changes; theme and seed
changes in Preferences; session restore after restart; external modification banner.
