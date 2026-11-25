//! Material 3 components drawn on top of egui primitives.
//!
//! egui's stock widgets are kept for text input internals only; everything the
//! user sees is painted here so the shape, state layers, elevation and type
//! scale follow Material 3.

use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Layout, Margin, Rect, Response, RichText, Sense,
    Stroke, TextEdit, Ui, Vec2,
};

use crate::theme::Theme;

pub const RADIUS_SMALL: f32 = 8.0;
pub const RADIUS_MEDIUM: f32 = 12.0;
pub const RADIUS_LARGE: f32 = 16.0;
pub const RADIUS_XL: f32 = 28.0;

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

pub fn icon(ui: &mut Ui, ch: char, size: f32, color: Color32) -> Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(ch.to_string(), icon_font(size), color));
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, galley, color);
    }
    response
}

/// Blend two colours in gamma space (Material state layers use alpha, but
/// opaque UI needs a colour).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        ch(a.r(), b.r()),
        ch(a.g(), b.g()),
        ch(a.b(), b.b()),
        ch(a.a(), b.a()),
    )
}

/// Faux elevation: a few progressively larger, fainter rounded rectangles.
pub fn soft_shadow(ui: &Ui, rect: egui::Rect, rounding: f32, color: Color32, spread: f32) {
    let painter = ui.painter();
    let alpha = color.a();
    for i in (1..=4).rev() {
        let grow = spread * i as f32 / 4.0;
        let a = ((alpha as f32) * 0.10 / i as f32) as u8;
        painter.rect_filled(
            rect.expand(grow).translate(Vec2::new(0.0, grow * 0.6)),
            rounding + grow,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a),
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Filled,
    Tonal,
    Outlined,
    Text,
    Elevated,
}

/// A Material button with optional leading icon.
pub fn button(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    leading: Option<char>,
    kind: ButtonKind,
) -> Response {
    let s = &theme.scheme;
    let text_font = theme.type_scale.label_large.clone();
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), text_font, Color32::PLACEHOLDER));
    let icon_size = 18.0;
    let pad_x = if kind == ButtonKind::Text { 12.0 } else { 20.0 };
    let mut width = galley.size().x + pad_x * 2.0;
    if leading.is_some() {
        width += icon_size + 8.0;
    }
    let height = 40.0;
    let size = Vec2::new(width.max(48.0), height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let rounding = height / 2.0;

    let (bg, fg, border) = match kind {
        ButtonKind::Filled => (s.primary, s.on_primary, None),
        ButtonKind::Tonal => (s.secondary_container, s.on_secondary_container, None),
        ButtonKind::Outlined => (Color32::TRANSPARENT, s.primary, Some(s.outline)),
        ButtonKind::Text => (Color32::TRANSPARENT, s.primary, None),
        ButtonKind::Elevated => (s.surface_container_low, s.primary, None),
    };
    let bg = if response.hovered() {
        match kind {
            ButtonKind::Filled => mix(bg, s.on_primary, 0.08),
            ButtonKind::Tonal | ButtonKind::Elevated => mix(bg, s.on_secondary_container, 0.08),
            _ => s.state_layer(s.primary, 20),
        }
    } else {
        bg
    };

    if ui.is_rect_visible(rect) {
        if kind == ButtonKind::Elevated {
            soft_shadow(ui, rect, rounding, s.shadow_color(theme.dark), 10.0);
        }
        let painter = ui.painter();
        if bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, rounding, bg);
        }
        if let Some(border) = border {
            painter.rect_stroke(rect, rounding, Stroke::new(1.0_f32, border));
        }
        let mut x = rect.center().x
            - (galley.size().x
                + if leading.is_some() {
                    icon_size + 8.0
                } else {
                    0.0
                })
                / 2.0;
        if let Some(ch) = leading {
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(x, rect.center().y - icon_size / 2.0),
                Vec2::splat(icon_size),
            );
            let g = ui.fonts(|f| f.layout_no_wrap(ch.to_string(), icon_font(icon_size), fg));
            painter.galley(
                egui::pos2(icon_rect.min.x, icon_rect.center().y - g.size().y / 2.0),
                g,
                fg,
            );
            x += icon_size + 8.0;
        }
        painter.galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            fg,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn filled_button(ui: &mut Ui, theme: &Theme, label: &str, icon: Option<char>) -> Response {
    button(ui, theme, label, icon, ButtonKind::Filled)
}

pub fn tonal_button(ui: &mut Ui, theme: &Theme, label: &str, icon: Option<char>) -> Response {
    button(ui, theme, label, icon, ButtonKind::Tonal)
}

pub fn text_button(ui: &mut Ui, theme: &Theme, label: &str) -> Response {
    button(ui, theme, label, None, ButtonKind::Text)
}

pub fn outlined_button(ui: &mut Ui, theme: &Theme, label: &str) -> Response {
    button(ui, theme, label, None, ButtonKind::Outlined)
}

/// 40x40 circular icon button with an optional selected (tonal) state.
pub fn icon_button(
    ui: &mut Ui,
    theme: &Theme,
    ch: char,
    tooltip: &str,
    selected: bool,
) -> Response {
    let s = &theme.scheme;
    let size = Vec2::splat(40.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hover = response.hovered();
    let bg = if selected {
        s.secondary_container
    } else if hover {
        s.state_layer(s.on_surface, 24)
    } else {
        Color32::TRANSPARENT
    };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if bg != Color32::TRANSPARENT {
            painter.circle_filled(rect.center(), 20.0, bg);
        }
        let color = if selected {
            s.on_secondary_container
        } else {
            s.on_surface_variant
        };
        let g = ui.fonts(|f| f.layout_no_wrap(ch.to_string(), icon_font(20.0), color));
        painter.galley(rect.center() - g.size() / 2.0, g, color);
    }
    response
        .on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// 28x28 circular icon button for the compact title bar.
pub fn icon_button_small(
    ui: &mut Ui,
    theme: &Theme,
    ch: char,
    tooltip: &str,
    selected: bool,
) -> Response {
    let s = &theme.scheme;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    let bg = if selected {
        s.secondary_container
    } else if response.hovered() {
        s.state_layer(s.on_surface, 24)
    } else {
        Color32::TRANSPARENT
    };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if bg != Color32::TRANSPARENT {
            painter.circle_filled(rect.center(), 14.0, bg);
        }
        let color = if selected {
            s.on_secondary_container
        } else {
            s.on_surface_variant
        };
        let g = ui.fonts(|f| f.layout_no_wrap(ch.to_string(), icon_font(16.0), color));
        painter.galley(rect.center() - g.size() / 2.0, g, color);
    }
    response
        .on_hover_text(tooltip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Filter / assist chip with a check mark when selected.
pub fn chip(ui: &mut Ui, theme: &Theme, label: &str, selected: bool) -> Response {
    let s = &theme.scheme;
    let font = theme.type_scale.label_large.clone();
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER));
    let height = 32.0;
    let width = galley.size().x + 24.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = if selected {
            s.secondary_container
        } else if response.hovered() {
            s.surface_container_highest
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, height / 2.0, bg);
        } else {
            painter.rect_stroke(rect, height / 2.0, Stroke::new(1.0_f32, s.outline));
        }
        let fg = if selected {
            s.on_secondary_container
        } else {
            s.on_surface_variant
        };
        painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Material switch. Returns the response; the value is toggled in place.
pub fn switch(ui: &mut Ui, theme: &Theme, id: egui::Id, on: &mut bool) -> Response {
    let s = &theme.scheme;
    let size = Vec2::new(52.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
    }
    let t = ui.ctx().animate_bool_with_time(id, *on, 0.14);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let track = mix(s.surface_container_highest, s.primary, t);
        painter.rect_filled(rect, 16.0, track);
        let knob_x = egui::lerp((rect.left() + 16.0)..=(rect.right() - 16.0), t);
        let knob_color = mix(s.outline, s.on_primary, t);
        let radius = egui::lerp(8.0..=12.0, t);
        painter.circle_filled(egui::pos2(knob_x, rect.center().y), radius, knob_color);
        if response.hovered() {
            painter.circle_filled(
                egui::pos2(knob_x, rect.center().y),
                radius + 8.0,
                s.state_layer(s.on_surface, 20),
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Filled text field with an optional caption above it.
pub fn text_field(
    ui: &mut Ui,
    theme: &Theme,
    id: egui::Id,
    value: &mut String,
    hint: &str,
    width: f32,
) -> Response {
    let s = &theme.scheme;
    let height = 44.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let focused = ui.memory(|m| m.has_focus(id));
    let hovered = ui.rect_contains_pointer(rect);
    let bg = if focused {
        s.surface_container_highest
    } else if hovered {
        s.surface_container_high
    } else {
        s.surface_container
    };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, RADIUS_SMALL + 2.0, bg);
        if focused {
            painter.rect_stroke(rect, RADIUS_SMALL + 2.0, Stroke::new(2.0_f32, s.primary));
        } else {
            painter.rect_stroke(
                rect,
                RADIUS_SMALL + 2.0,
                Stroke::new(1.0_f32, s.outline_variant),
            );
        }
    }
    let inner = rect.shrink2(Vec2::new(14.0, 6.0));
    let mut child = ui.child_ui(inner, Layout::left_to_right(Align::Center));
    let response = child.add(
        TextEdit::singleline(value)
            .id(id)
            .frame(false)
            .margin(Margin::ZERO)
            .font(theme.type_scale.body_medium.clone())
            .text_color(s.on_surface)
            .hint_text(RichText::new(hint).color(s.outline))
            .desired_width(inner.width()),
    );
    response
}

/// One tab in the document strip.
pub struct TabResponse {
    pub response: Response,
    pub close_clicked: bool,
}

/// Compact tab for the thin title bar: no pill, an underline marks the active one.
pub fn tab_compact(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    dirty: bool,
    active: bool,
    closable: bool,
) -> TabResponse {
    let s = &theme.scheme;
    let font = theme.type_scale.label_large.clone();
    let close_w = if closable { 20.0 } else { 0.0 };
    // The tab has a hard maximum width, so the title is laid out into the space
    // that is actually left and elided; painting the full-length galley made long
    // names run past the tab background.
    let max_text: f32 = 200.0 - 22.0 - close_w;
    let galley = ui.fonts(|f| {
        let mut job = egui::text::LayoutJob::simple_singleline(
            title.to_owned(),
            font.clone(),
            Color32::PLACEHOLDER,
        );
        job.wrap = egui::text::TextWrapping {
            max_width: max_text.max(24.0),
            max_rows: 1,
            break_anywhere: false,
            overflow_character: Some('…'),
        };
        f.layout_job(job)
    });
    let width = (galley.size().x + 22.0 + close_w).min(200.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    let mut close_clicked = false;
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let bg = if active {
            s.surface_container_high
        } else if response.hovered() {
            s.surface_container
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            painter.rect_filled(rect, RADIUS_SMALL, bg);
        }
        let fg = if active {
            s.on_surface
        } else {
            s.on_surface_variant
        };
        painter.galley(
            egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
            galley.clone(),
            fg,
        );
        if active {
            let underline = Rect::from_min_max(
                egui::pos2(rect.left() + 10.0, rect.bottom() - 2.0),
                egui::pos2(rect.right() - 10.0, rect.bottom()),
            );
            painter.rect_filled(underline, 1.0, s.primary);
        }
        let close_center = egui::pos2(rect.right() - 12.0, rect.center().y);
        if closable {
            let hovered = ui
                .ctx()
                .pointer_hover_pos()
                .map(|p| (p - close_center).length() < 10.0)
                .unwrap_or(false);
            if hovered {
                painter.circle_filled(close_center, 9.0, s.state_layer(s.on_surface, 30));
            }
            let close_color = if hovered { s.on_surface } else { fg };
            let g = ui.fonts(|f| {
                f.layout_no_wrap(
                    crate::icons::CLOSE.to_string(),
                    icon_font(12.0),
                    close_color,
                )
            });
            painter.galley(close_center - g.size() / 2.0, g, close_color);
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    close_clicked = (pos - close_center).length() < 11.0;
                }
            }
        }
        if dirty {
            painter.circle_filled(
                egui::pos2(rect.left() + 5.0, rect.center().y),
                2.0,
                s.primary,
            );
        }
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    TabResponse {
        response,
        close_clicked,
    }
}

/// A row inside a menu.
pub fn menu_item(
    ui: &mut Ui,
    theme: &Theme,
    icon: Option<char>,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> Response {
    let s = &theme.scheme;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::click());
    if !enabled {
        // Keep the row but paint it muted; egui responses cannot be disabled after allocation.
    }
    let hovered = response.hovered() && enabled;
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if hovered {
            painter.rect_filled(rect, RADIUS_SMALL, s.state_layer(s.primary, 22));
        }
        let fg = if enabled {
            s.on_surface
        } else {
            s.on_surface_variant.gamma_multiply(0.5)
        };
        let mut x = rect.left() + 12.0;
        if let Some(ch) = icon {
            let g = ui.fonts(|f| f.layout_no_wrap(ch.to_string(), icon_font(18.0), fg));
            painter.galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
            x += 26.0;
        }
        let font = theme.type_scale.body_medium.clone();
        let g = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER));
        painter.galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
        if let Some(shortcut) = shortcut {
            let font = theme.type_scale.label_small.clone();
            let g = ui.fonts(|f| f.layout_no_wrap(shortcut.to_owned(), font, s.on_surface_variant));
            painter.galley(
                egui::pos2(
                    rect.right() - 12.0 - g.size().x,
                    rect.center().y - g.size().y / 2.0,
                ),
                g,
                s.on_surface_variant,
            );
        }
    }
    response.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::Default
    })
}

pub fn divider(ui: &mut Ui, theme: &Theme) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, theme.scheme.outline_variant);
}

/// The application mark: a tonal rounded square with document lines.
pub fn logo(ui: &mut Ui, theme: &Theme, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, size * 0.28, theme.scheme.primary);
    let line_color = theme.scheme.on_primary;
    let pad = size * 0.26;
    let widths = [1.0_f32, 0.62, 0.82];
    for (i, factor) in widths.iter().enumerate() {
        let y = rect.top() + pad + (size - pad * 2.0) * (i as f32 + 1.0) / 4.0;
        let w = (size - pad * 2.0) * factor;
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad, y),
                Vec2::new(w.max(2.0), size * 0.055),
            ),
            size * 0.03,
            Color32::from_rgba_unmultiplied(
                line_color.r(),
                line_color.g(),
                line_color.b(),
                if i == 0 { 235 } else { 150 },
            ),
        );
    }
}

/// Wraps a closure in a clipped, scrollable region used by side panels.
pub fn body_text(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text)
            .font(theme.type_scale.body_medium.clone())
            .color(theme.scheme.on_surface_variant),
    );
}

/// Small label used for metadata rows (status bar, list rows).
pub fn caption(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    ui.label(
        RichText::new(text)
            .font(theme.type_scale.label_small.clone())
            .color(color),
    );
}
