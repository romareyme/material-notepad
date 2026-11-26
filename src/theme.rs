//! Material You (Material Design 3) color and type system.
//!
//! Palettes are generated from a single seed colour in OKLCH.  HCT (the space
//! used by the canonical Material color utilities) and OKLCH agree closely on
//! hue and tone, and OKLCH has a well behaved perceptual lightness, so tonal
//! palettes built here behave like the reference implementation while staying
//! dependency free.

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke};

/// Material tones used by the light and dark schemes.
#[allow(dead_code)]
pub mod tone {
    pub const T4: u8 = 4;
    pub const T6: u8 = 6;
    pub const T10: u8 = 10;
    pub const T12: u8 = 12;
    pub const T17: u8 = 17;
    pub const T20: u8 = 20;
    pub const T22: u8 = 22;
    pub const T24: u8 = 24;
    pub const T30: u8 = 30;
    pub const T40: u8 = 40;
    pub const T50: u8 = 50;
    pub const T60: u8 = 60;
    pub const T80: u8 = 80;
    pub const T87: u8 = 87;
    pub const T90: u8 = 90;
    pub const T92: u8 = 92;
    pub const T94: u8 = 94;
    pub const T95: u8 = 95;
    pub const T96: u8 = 96;
    pub const T98: u8 = 98;
    pub const T100: u8 = 100;
}

/// A point in OKLCH space. `l` is 0..=1, `c` is 0..~0.4, `h` is degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

impl Oklch {
    pub fn new(l: f32, c: f32, h: f32) -> Self {
        Self { l, c, h }
    }

    pub fn to_color32(self) -> Color32 {
        let h = self.h.to_radians();
        let (sin_h, cos_h) = h.sin_cos();
        let a = self.c * cos_h;
        let b = self.c * sin_h;
        let rgb = oklab_to_linear(self.l, a, b);
        let (r, g, b) = gamut_map(rgb, self.l, self.c, self.h);
        Color32::from_rgb(linear_to_u8(r), linear_to_u8(g), linear_to_u8(b))
    }
}

/// Convert an sRGB colour into OKLCH.
pub fn color_to_oklch(color: Color32) -> Oklch {
    let r = srgb_to_linear(color.r() as f32 / 255.0);
    let g = srgb_to_linear(color.g() as f32 / 255.0);
    let b = srgb_to_linear(color.b() as f32 / 255.0);
    let (l, a, bb) = linear_to_oklab(r, g, b);
    let c = (a * a + bb * bb).sqrt();
    let mut h = bb.atan2(a).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    Oklch::new(l, c, h)
}

fn oklab_to_linear(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;
    (
        4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3,
        -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3,
    )
}

fn linear_to_oklab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();
    (
        0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
        1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    )
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn linear_to_u8(v: f32) -> u8 {
    (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8
}

/// Reduce chroma until the colour fits in sRGB, keeping lightness and hue.
fn gamut_map(rgb: (f32, f32, f32), l: f32, c: f32, h: f32) -> (f32, f32, f32) {
    let in_gamut = |v: (f32, f32, f32)| {
        (-0.000_5..=1.000_5).contains(&v.0)
            && (-0.000_5..=1.000_5).contains(&v.1)
            && (-0.000_5..=1.000_5).contains(&v.2)
    };
    if in_gamut(rgb) || c <= 0.000_1 {
        return (
            rgb.0.clamp(0.0, 1.0),
            rgb.1.clamp(0.0, 1.0),
            rgb.2.clamp(0.0, 1.0),
        );
    }
    let (mut lo, mut hi) = (0.0_f32, c);
    let mut best = (l, 0.0, 0.0);
    let rad = h.to_radians();
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        let (a, b) = (mid * rad.cos(), mid * rad.sin());
        let candidate = oklab_to_linear(l, a, b);
        if in_gamut(candidate) {
            lo = mid;
            best = candidate;
        } else {
            hi = mid;
        }
    }
    (
        best.0.clamp(0.0, 1.0),
        best.1.clamp(0.0, 1.0),
        best.2.clamp(0.0, 1.0),
    )
}

/// Material tone is CIE L*; OKLCH lightness is roughly the cube root of the
/// relative luminance, so map through L* -> Y -> cbrt(Y).  This keeps the
/// familiar Material tonal steps (6, 10, 30, 90, 98) perceptually correct.
pub fn tone_to_lightness(tone: u8) -> f32 {
    let lstar = tone as f32;
    let y = if lstar > 8.0 {
        ((lstar + 16.0) / 116.0).powi(3)
    } else {
        lstar / 903.3
    };
    y.max(0.0).cbrt()
}

/// Build a tonal palette (tone -> colour) from a seed.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub hue: f32,
    pub chroma: f32,
}

impl Palette {
    pub fn new(hue: f32, chroma: f32) -> Self {
        Self { hue, chroma }
    }

    pub fn at(&self, tone: u8) -> Color32 {
        Oklch::new(tone_to_lightness(tone), self.chroma, self.hue).to_color32()
    }
}

/// The complete set of Material 3 color roles. Some roles are part of the
/// specification without being referenced by the current widgets.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Scheme {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,

    pub secondary: Color32,
    pub on_secondary: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,

    pub tertiary: Color32,
    pub on_tertiary: Color32,
    pub tertiary_container: Color32,
    pub on_tertiary_container: Color32,

    pub error: Color32,
    pub on_error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,

    pub background: Color32,
    pub on_background: Color32,
    pub surface: Color32,
    pub on_surface: Color32,
    pub surface_variant: Color32,
    pub on_surface_variant: Color32,

    pub surface_dim: Color32,
    pub surface_bright: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,

    pub outline: Color32,
    pub outline_variant: Color32,
    pub shadow: Color32,
    pub scrim: Color32,
    pub inverse_surface: Color32,
    pub inverse_on_surface: Color32,
    pub inverse_primary: Color32,
}

impl Scheme {
    pub fn light(seed: Color32) -> Self {
        let seed_lch = color_to_oklch(seed);
        let primary = Palette::new(seed_lch.h, seed_lch.c.clamp(0.075, 0.155));
        let secondary = Palette::new(seed_lch.h, 0.055);
        let tertiary = Palette::new((seed_lch.h + 55.0) % 360.0, 0.085);
        let neutral = Palette::new(seed_lch.h, 0.008);
        let neutral_variant = Palette::new(seed_lch.h, 0.018);
        let error = Palette::new(25.0, 0.16);

        Self {
            primary: primary.at(tone::T40),
            on_primary: primary.at(tone::T100),
            primary_container: primary.at(tone::T90),
            on_primary_container: primary.at(tone::T10),

            secondary: secondary.at(tone::T40),
            on_secondary: secondary.at(tone::T100),
            secondary_container: secondary.at(tone::T90),
            on_secondary_container: secondary.at(tone::T10),

            tertiary: tertiary.at(tone::T40),
            on_tertiary: tertiary.at(tone::T100),
            tertiary_container: tertiary.at(tone::T90),
            on_tertiary_container: tertiary.at(tone::T10),

            error: error.at(tone::T40),
            on_error: error.at(tone::T100),
            error_container: error.at(tone::T90),
            on_error_container: error.at(tone::T10),

            background: neutral.at(tone::T98),
            on_background: neutral.at(tone::T10),
            surface: neutral.at(tone::T98),
            on_surface: neutral.at(tone::T10),
            surface_variant: neutral_variant.at(tone::T90),
            on_surface_variant: neutral_variant.at(tone::T30),

            surface_dim: neutral.at(tone::T87),
            surface_bright: neutral.at(tone::T98),
            surface_container_lowest: neutral.at(tone::T100),
            surface_container_low: neutral.at(tone::T96),
            surface_container: neutral.at(tone::T94),
            surface_container_high: neutral.at(tone::T92),
            surface_container_highest: neutral.at(tone::T90),

            outline: neutral_variant.at(tone::T50),
            outline_variant: neutral_variant.at(tone::T80),
            shadow: Color32::BLACK,
            scrim: Color32::BLACK,
            inverse_surface: neutral.at(tone::T20),
            inverse_on_surface: neutral.at(tone::T95),
            inverse_primary: primary.at(tone::T80),
        }
    }

    pub fn dark(seed: Color32) -> Self {
        let seed_lch = color_to_oklch(seed);
        let primary = Palette::new(seed_lch.h, seed_lch.c.clamp(0.06, 0.14));
        let secondary = Palette::new(seed_lch.h, 0.05);
        let tertiary = Palette::new((seed_lch.h + 55.0) % 360.0, 0.08);
        let neutral = Palette::new(seed_lch.h, 0.008);
        let neutral_variant = Palette::new(seed_lch.h, 0.018);
        let error = Palette::new(25.0, 0.15);

        Self {
            primary: primary.at(tone::T80),
            on_primary: primary.at(tone::T20),
            primary_container: primary.at(tone::T30),
            on_primary_container: primary.at(tone::T90),

            secondary: secondary.at(tone::T80),
            on_secondary: secondary.at(tone::T20),
            secondary_container: secondary.at(tone::T30),
            on_secondary_container: secondary.at(tone::T90),

            tertiary: tertiary.at(tone::T80),
            on_tertiary: tertiary.at(tone::T20),
            tertiary_container: tertiary.at(tone::T30),
            on_tertiary_container: tertiary.at(tone::T90),

            error: error.at(tone::T80),
            on_error: error.at(tone::T20),
            error_container: error.at(tone::T30),
            on_error_container: error.at(tone::T80),

            background: neutral.at(tone::T6),
            on_background: neutral.at(tone::T90),
            surface: neutral.at(tone::T6),
            on_surface: neutral.at(tone::T90),
            surface_variant: neutral_variant.at(tone::T30),
            on_surface_variant: neutral_variant.at(tone::T80),

            surface_dim: neutral.at(tone::T6),
            surface_bright: neutral.at(tone::T24),
            surface_container_lowest: neutral.at(tone::T4),
            surface_container_low: neutral.at(tone::T10),
            surface_container: neutral.at(tone::T12),
            surface_container_high: neutral.at(tone::T17),
            surface_container_highest: neutral.at(tone::T22),

            outline: neutral_variant.at(tone::T60),
            outline_variant: neutral_variant.at(tone::T30),
            shadow: Color32::BLACK,
            scrim: Color32::BLACK,
            inverse_surface: neutral.at(tone::T90),
            inverse_on_surface: neutral.at(tone::T20),
            inverse_primary: primary.at(tone::T40),
        }
    }

    /// Translucent state layer (hover / focus / press) over a surface.
    pub fn state_layer(&self, base: Color32, opacity: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(
            base.r(),
            base.g(),
            base.b(),
            ((base.a() as u16 * opacity as u16) / 255) as u8,
        )
    }

    pub fn with_alpha(color: Color32, opacity: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), opacity)
    }

    pub fn shadow_color(&self, dark: bool) -> Color32 {
        if dark {
            Color32::from_black_alpha(160)
        } else {
            Color32::from_black_alpha(40)
        }
    }
}

/// Material 3 type scale, expressed in points at the current UI scale. Some
/// steps of the scale are part of the specification without being used yet.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Typography {
    pub display_small: FontId,
    pub headline_small: FontId,
    pub title_large: FontId,
    pub title_medium: FontId,
    pub title_small: FontId,
    pub body_large: FontId,
    pub body_medium: FontId,
    pub body_small: FontId,
    pub label_large: FontId,
    pub label_medium: FontId,
    pub label_small: FontId,
    pub mono: FontId,
    pub editor_scale: f32,
}

impl Typography {
    pub fn new(scale: f32, editor_font_size: f32) -> Self {
        let f = |size: f32| FontId::new((size * scale).round(), FontFamily::Proportional);
        Self {
            display_small: f(36.0),
            headline_small: f(24.0),
            title_large: f(22.0),
            title_medium: f(16.0),
            title_small: f(14.0),
            body_large: f(16.0),
            body_medium: f(14.0),
            body_small: f(12.0),
            label_large: f(14.0),
            label_medium: f(12.0),
            label_small: f(11.0),
            mono: FontId::new((editor_font_size * scale).round(), FontFamily::Monospace),
            editor_scale: scale,
        }
    }
}

/// The active theme handed to every widget.
#[derive(Clone, Debug)]
pub struct Theme {
    pub dark: bool,
    #[allow(dead_code)]
    pub seed: Color32,
    pub scheme: Scheme,
    pub syntax: SyntaxColors,
    pub type_scale: Typography,
    pub editor_font_size: f32,
    pub editor_line_height: f32,
}

impl Theme {
    pub fn new(dark: bool, seed: Color32, scale: f32, editor_font_size: f32) -> Self {
        let scheme = if dark {
            Scheme::dark(seed)
        } else {
            Scheme::light(seed)
        };
        let syntax = compute_syntax_colors(dark, seed, scheme);
        let line_height = (editor_font_size * scale * 1.55).round();
        Self {
            dark,
            seed,
            scheme,
            syntax,
            type_scale: Typography::new(scale, editor_font_size),
            editor_font_size: editor_font_size * scale,
            editor_line_height: line_height,
        }
    }

    pub fn editor_font(&self) -> FontId {
        FontId::new(self.editor_font_size, FontFamily::Monospace)
    }

    /// Colors for syntax highlighting; readable on `surface_container_lowest`.
    pub fn syntax(&self) -> SyntaxColors {
        self.syntax
    }

    /// Install fonts and global egui visuals for this theme.
    pub fn apply(&self, ctx: &egui::Context) {
        let s = self.scheme;
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = self.dark;
        style.visuals.override_text_color = Some(s.on_surface);
        style.visuals.panel_fill = s.surface;
        style.visuals.window_fill = s.surface_container;
        style.visuals.extreme_bg_color = s.surface_container_lowest;
        style.visuals.faint_bg_color = s.surface_container_low;
        style.visuals.code_bg_color = s.surface_container_high;
        style.visuals.hyperlink_color = s.primary;
        style.visuals.warn_fg_color = s.tertiary;
        style.visuals.error_fg_color = s.error;
        style.visuals.selection.bg_fill = Scheme::with_alpha(s.primary, 70);
        style.visuals.selection.stroke = Stroke::new(1.0_f32, s.on_surface);
        style.visuals.text_cursor = Stroke::new(2.0_f32, s.primary);
        style.visuals.window_rounding = egui::Rounding::same(24.0);
        style.visuals.menu_rounding = egui::Rounding::same(12.0);
        style.visuals.window_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 28.0,
            spread: 0.0,
            color: Color32::from_black_alpha(if self.dark { 150 } else { 40 }),
        };
        style.visuals.popup_shadow = style.visuals.window_shadow;
        style.visuals.widgets.noninteractive.bg_fill = s.surface_container;
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, s.on_surface_variant);
        style.visuals.widgets.inactive.bg_fill = s.surface_container_high;
        style.visuals.widgets.inactive.weak_bg_fill = s.surface_container_high;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, s.on_surface);
        style.visuals.widgets.hovered.bg_fill = s.surface_container_highest;
        style.visuals.widgets.hovered.weak_bg_fill = s.surface_container_highest;
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, s.on_surface);
        style.visuals.widgets.active.bg_fill = s.surface_container_highest;
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, s.on_surface);
        style.visuals.widgets.open.bg_fill = s.surface_container_high;
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, s.on_surface);
        for w in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            w.rounding = egui::Rounding::same(10.0);
            w.expansion = 0.0;
        }
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 36.0;
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.spacing.scroll.bar_inner_margin = 4.0;
        style.spacing.scroll.bar_outer_margin = 2.0;
        style.spacing.scroll.handle_min_length = 24.0;
        style.animation_time = 0.16;
        ctx.set_style(style);
    }
}

/// Syntax spotlight colours.
#[derive(Clone, Copy, Debug)]
pub struct SyntaxColors {
    pub text: Color32,
    pub keyword: Color32,
    pub type_name: Color32,
    pub function: Color32,
    pub string: Color32,
    pub number: Color32,
    pub comment: Color32,
    pub operator: Color32,
    pub punctuation: Color32,
    pub preprocessor: Color32,
    pub constant: Color32,
    pub attribute: Color32,
    pub tag: Color32,
    pub heading: Color32,
    pub link: Color32,
    pub invalid: Color32,
}

fn compute_syntax_colors(dark: bool, seed: Color32, s: Scheme) -> SyntaxColors {
    let h = color_to_oklch(seed).h;
    let hue = |offset: f32| (h + offset).rem_euclid(360.0);
    let tone = |offset: f32, chroma: f32, lightness: f32| {
        Oklch::new(lightness, chroma, hue(offset)).to_color32()
    };
    let l = if dark { 0.82 } else { 0.46 };
    SyntaxColors {
        text: s.on_surface,
        keyword: tone(0.0, 0.15, l),
        type_name: tone(52.0, 0.10, if dark { 0.84 } else { 0.44 }),
        function: tone(310.0, 0.12, if dark { 0.85 } else { 0.45 }),
        string: Oklch::new(if dark { 0.79 } else { 0.45 }, 0.12, 155.0).to_color32(),
        number: Oklch::new(if dark { 0.83 } else { 0.47 }, 0.13, 75.0).to_color32(),
        comment: if dark {
            Color32::from_gray(138)
        } else {
            Color32::from_gray(118)
        },
        operator: s.on_surface_variant,
        punctuation: s.on_surface_variant,
        preprocessor: tone(20.0, 0.12, if dark { 0.80 } else { 0.45 }),
        constant: tone(330.0, 0.14, if dark { 0.82 } else { 0.46 }),
        attribute: Oklch::new(if dark { 0.82 } else { 0.47 }, 0.11, 95.0).to_color32(),
        tag: tone(8.0, 0.14, if dark { 0.80 } else { 0.45 }),
        heading: tone(0.0, 0.15, if dark { 0.87 } else { 0.42 }),
        link: Oklch::new(if dark { 0.80 } else { 0.46 }, 0.13, 235.0).to_color32(),
        invalid: s.error,
    }
}

/// Seed palettes offered in preferences (Material You style colour choices).
pub const SEEDS: [(&str, Color32); 8] = [
    ("Lavender", Color32::from_rgb(0x67, 0x50, 0xA4)),
    ("Ocean", Color32::from_rgb(0x00, 0x63, 0x9B)),
    ("Lagoon", Color32::from_rgb(0x00, 0x6A, 0x6A)),
    ("Forest", Color32::from_rgb(0x3B, 0x69, 0x39)),
    ("Ember", Color32::from_rgb(0x8B, 0x50, 0x00)),
    ("Rose", Color32::from_rgb(0x8E, 0x45, 0x85)),
    ("Slate", Color32::from_rgb(0x4A, 0x5C, 0x92)),
    ("Gold", Color32::from_rgb(0x6F, 0x5D, 0x00)),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        0.2126 * srgb_to_linear(c.r() as f32 / 255.0)
            + 0.7152 * srgb_to_linear(c.g() as f32 / 255.0)
            + 0.0722 * srgb_to_linear(c.b() as f32 / 255.0)
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (l1, l2) = (luminance(a), luminance(b));
        let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn round_trips_primary_colours() {
        for (_, seed) in SEEDS {
            let lch = color_to_oklch(seed);
            let back = Oklch::new(lch.l, lch.c, lch.h).to_color32();
            let dr = (back.r() as i32 - seed.r() as i32).abs();
            let dg = (back.g() as i32 - seed.g() as i32).abs();
            let db = (back.b() as i32 - seed.b() as i32).abs();
            assert!(dr <= 2 && dg <= 2 && db <= 2, "seed {seed:?} -> {back:?}");
        }
    }

    #[test]
    fn schemes_meet_contrast() {
        for (_, seed) in SEEDS {
            for dark in [false, true] {
                let s = if dark {
                    Scheme::dark(seed)
                } else {
                    Scheme::light(seed)
                };
                for (fg, bg, min) in [
                    (s.on_surface, s.surface, 7.0),
                    (s.on_primary, s.primary, 4.5),
                    (s.on_primary_container, s.primary_container, 4.5),
                    (s.on_surface_variant, s.surface_container, 4.5),
                    (s.on_error, s.error, 4.5),
                ] {
                    let ratio = contrast(fg, bg);
                    assert!(
                        ratio >= min,
                        "contrast {ratio:.2} < {min} for {fg:?} on {bg:?} (dark={dark}, seed={seed:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn tonal_palette_is_monotonic() {
        let p = Palette::new(280.0, 0.12);
        let mut last = -1.0;
        for t in 0..=100u8 {
            let lum = luminance(p.at(t));
            assert!(lum >= last - 0.001, "tone {t} darker than previous");
            last = lum;
        }
    }
}
