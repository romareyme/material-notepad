# Vendored egui-winit

This is egui-winit 0.27.2 with one local change in src/lib.rs (search for
Notepad Next patch):

`ust
if let Some(logical_key) = logical_key.or(physical_key) {
`

winit reports Ctrl + <letter> as a control character on Windows, and on non-Latin
keyboard layouts the logical key can have no egui::Key mapping. Upstream drops such
events entirely, which breaks every Ctrl shortcut on a Cyrillic layout. Falling back to
the physical key keeps shortcuts layout independent.

Upstream: <https://github.com/emilk/egui> (MIT/Apache-2.0).
