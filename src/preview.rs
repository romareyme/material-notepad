//! Rich preview panels: Markdown live preview, CSV table view, and side-by-side/unified Diff.

use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui, Vec2};

use crate::theme::Theme;
use crate::transform::{line_diff, parse_csv, DiffKind};
use crate::widgets;

/// Render Markdown text using Material 3 typography and palette.
pub fn show_markdown_preview(ui: &mut Ui, theme: &Theme, text: &str) {
    let s = theme.scheme;
    Frame::none()
        .fill(s.surface_container_low)
        .inner_margin(Margin::symmetric(20.0, 14.0))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_source("md-preview-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut in_code = false;
                    let mut code_buf = String::new();
                    for line in text.lines() {
                        let trimmed = line.trim_start();
                        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                            if in_code {
                                render_code_block(ui, theme, &code_buf);
                                code_buf.clear();
                                in_code = false;
                            } else {
                                in_code = true;
                            }
                            continue;
                        }
                        if in_code {
                            if !code_buf.is_empty() {
                                code_buf.push('\n');
                            }
                            code_buf.push_str(line);
                            continue;
                        }
                        if trimmed.is_empty() {
                            ui.add_space(6.0);
                            continue;
                        }
                        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                            ui.add_space(4.0);
                            widgets::divider(ui, theme);
                            ui.add_space(4.0);
                            continue;
                        }
                        if let Some(rest) = trimmed.strip_prefix("# ") {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(rest)
                                    .font(theme.type_scale.headline_small.clone())
                                    .color(s.primary),
                            );
                            ui.add_space(2.0);
                            continue;
                        }
                        if let Some(rest) = trimmed.strip_prefix("## ") {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(rest)
                                    .font(theme.type_scale.title_large.clone())
                                    .color(s.on_surface),
                            );
                            continue;
                        }
                        if trimmed.starts_with("### ")
                            || trimmed.starts_with("#### ")
                            || trimmed.starts_with("##### ")
                            || trimmed.starts_with("###### ")
                        {
                            let rest = trimmed.trim_start_matches('#').trim_start();
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(rest)
                                    .font(theme.type_scale.title_medium.clone())
                                    .color(s.on_surface),
                            );
                            continue;
                        }
                        if let Some(quote) = trimmed.strip_prefix("> ") {
                            Frame::none()
                                .fill(s.surface_container)
                                .rounding(widgets::RADIUS_SMALL)
                                .inner_margin(Margin::symmetric(12.0, 6.0))
                                .stroke(Stroke::new(1.0_f32, s.outline_variant))
                                .show(ui, |ui| {
                                    render_inline_markdown(ui, theme, quote, s.on_surface_variant);
                                });
                            continue;
                        }
                        if let Some(item) = trimmed
                            .strip_prefix("- ")
                            .or_else(|| trimmed.strip_prefix("* "))
                            .or_else(|| trimmed.strip_prefix("+ "))
                        {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new("•")
                                        .font(theme.type_scale.body_medium.clone())
                                        .color(s.primary),
                                );
                                render_inline_markdown(ui, theme, item, s.on_surface);
                            });
                            continue;
                        }
                        render_inline_markdown(ui, theme, line, s.on_surface);
                    }
                    if in_code && !code_buf.is_empty() {
                        render_code_block(ui, theme, &code_buf);
                    }
                });
        });
}

fn render_code_block(ui: &mut Ui, theme: &Theme, code: &str) {
    let s = theme.scheme;
    ui.add_space(4.0);
    Frame::none()
        .fill(s.surface_container_highest)
        .rounding(widgets::RADIUS_SMALL)
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new(code)
                    .font(theme.editor_font())
                    .color(s.on_surface),
            );
        });
    ui.add_space(4.0);
}

fn render_inline_markdown(ui: &mut Ui, theme: &Theme, text: &str, base_color: Color32) {
    let s = theme.scheme;
    let mut job = egui::text::LayoutJob::default();
    let normal = egui::TextFormat {
        font_id: theme.type_scale.body_medium.clone(),
        color: base_color,
        ..Default::default()
    };
    let bold = egui::TextFormat {
        font_id: egui::FontId::new(
            theme.type_scale.body_medium.size,
            egui::FontFamily::Name("roboto-medium".into()),
        ),
        color: base_color,
        ..Default::default()
    };
    let code_fmt = egui::TextFormat {
        font_id: theme.type_scale.mono.clone(),
        color: s.primary,
        background: s.surface_container_highest,
        ..Default::default()
    };
    let link_fmt = egui::TextFormat {
        font_id: theme.type_scale.body_medium.clone(),
        color: s.primary,
        underline: Stroke::new(1.0_f32, s.primary),
        ..Default::default()
    };

    let mut i = 0usize;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if text[i..].starts_with("**") {
            if let Some(end) = text[i + 2..].find("**") {
                job.append(&text[i + 2..i + 2 + end], 0.0, bold.clone());
                i += 2 + end + 2;
                continue;
            }
        }
        if text[i..].starts_with('`') {
            if let Some(end) = text[i + 1..].find('`') {
                job.append(&text[i + 1..i + 1 + end], 0.0, code_fmt.clone());
                i += 1 + end + 1;
                continue;
            }
        }
        if text[i..].starts_with('[') {
            if let Some(close) = text[i..].find(']') {
                let after = i + close + 1;
                if text[after..].starts_with('(') {
                    if let Some(paren) = text[after..].find(')') {
                        let label = &text[i + 1..i + close];
                        job.append(label, 0.0, link_fmt.clone());
                        i = after + paren + 1;
                        continue;
                    }
                }
            }
        }
        let next = text[i..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        job.append(&text[i..i + next], 0.0, normal.clone());
        i += next;
    }
    ui.label(job);
}

/// Render CSV/TSV text as a scrollable Material table.
pub fn show_csv_preview(ui: &mut Ui, theme: &Theme, text: &str) {
    let s = theme.scheme;
    let rows = parse_csv(text);
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let syntax = theme.syntax();
    let palette = [
        syntax.text,
        syntax.keyword,
        syntax.string,
        syntax.number,
        syntax.function,
        syntax.type_name,
        syntax.constant,
    ];

    Frame::none()
        .fill(s.surface_container_low)
        .inner_margin(Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            if rows.is_empty() || cols == 0 {
                widgets::body_text(ui, theme, "Empty CSV table.");
                return;
            }
            ScrollArea::both()
                .id_source("csv-preview-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("csv-table-grid")
                        .striped(true)
                        .spacing(Vec2::new(16.0, 6.0))
                        .show(ui, |ui| {
                            for (r_idx, row) in rows.iter().take(500).enumerate() {
                                for c_idx in 0..cols {
                                    let val = row.get(c_idx).map(String::as_str).unwrap_or("");
                                    let color = if r_idx == 0 {
                                        s.primary
                                    } else {
                                        palette[c_idx % palette.len()]
                                    };
                                    let font = if r_idx == 0 {
                                        theme.type_scale.label_large.clone()
                                    } else {
                                        theme.type_scale.mono.clone()
                                    };
                                    ui.label(RichText::new(val).font(font).color(color));
                                }
                                ui.end_row();
                            }
                        });
                    if rows.len() > 500 {
                        ui.add_space(6.0);
                        widgets::caption(
                            ui,
                            theme,
                            &format!("Showing first 500 of {} rows", rows.len()),
                            s.outline,
                        );
                    }
                });
        });
}

/// Render a unified line diff between two documents.
pub fn show_diff_view(
    ui: &mut Ui,
    theme: &Theme,
    left_title: &str,
    left_text: &str,
    right_title: &str,
    right_text: &str,
) {
    let s = theme.scheme;
    let diff = line_diff(left_text, right_text);
    let added = diff.iter().filter(|d| d.kind == DiffKind::Added).count();
    let removed = diff.iter().filter(|d| d.kind == DiffKind::Removed).count();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{left_title}  ↔  {right_title}"))
                .font(theme.type_scale.title_small.clone())
                .color(s.on_surface),
        );
        ui.add_space(12.0);
        widgets::caption(
            ui,
            theme,
            &format!("+{added}  -{removed}"),
            s.primary,
        );
    });
    ui.add_space(4.0);
    widgets::divider(ui, theme);
    ui.add_space(4.0);

    let add_bg = Color32::from_rgba_unmultiplied(60, 180, 100, if theme.dark { 42 } else { 36 });
    let rem_bg = Color32::from_rgba_unmultiplied(220, 75, 75, if theme.dark { 45 } else { 36 });

    ScrollArea::both()
        .id_source("diff-view-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for item in &diff {
                let (prefix, bg, fg) = match item.kind {
                    DiffKind::Same => (" ", Color32::TRANSPARENT, s.on_surface_variant),
                    DiffKind::Added => ("+", add_bg, s.on_surface),
                    DiffKind::Removed => ("-", rem_bg, s.on_surface),
                };
                let lno = item
                    .left_line
                    .map(|n| format!("{n:>4}"))
                    .unwrap_or_else(|| "    ".to_owned());
                let rno = item
                    .right_line
                    .map(|n| format!("{n:>4}"))
                    .unwrap_or_else(|| "    ".to_owned());
                Frame::none().fill(bg).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{lno} {rno} {prefix}"))
                                .font(theme.editor_font())
                                .color(s.outline),
                        );
                        ui.label(
                            RichText::new(&item.text)
                                .font(theme.editor_font())
                                .color(fg),
                        );
                    });
                });
            }
        });
}
