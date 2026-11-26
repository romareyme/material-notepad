#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod document;
mod editor;
mod files;
mod highlight;
mod icons;
mod search;
mod session;
mod theme;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let settings = session::load_settings();
    let files: Vec<std::path::PathBuf> = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_file())
        .collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Material Notepad")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 620.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Material Notepad",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Box::new(app::NotepadApp::new(cc, settings, files))
        }),
    )
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "roboto".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Roboto-Regular.ttf")),
    );
    fonts.font_data.insert(
        "roboto-medium".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Roboto-Medium.ttf")),
    );
    fonts.font_data.insert(
        "roboto-mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/RobotoMono-Regular.ttf")),
    );
    fonts.font_data.insert(
        "roboto-mono-bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/RobotoMono-Bold.ttf")),
    );
    fonts.font_data.insert(
        "icons".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/MaterialSymbolsRounded.ttf")),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "roboto".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("icons".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "roboto-mono".to_owned());
    fonts.families.insert(
        egui::FontFamily::Name("icons".into()),
        vec!["icons".to_owned(), "roboto".to_owned()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("roboto-medium".into()),
        vec![
            "roboto-medium".to_owned(),
            "icons".to_owned(),
            "roboto".to_owned(),
        ],
    );
    ctx.set_fonts(fonts);
}

/// Generate the window icon: the same Material You document as `assets/icon.ico`
/// — a white sheet with a folded corner, drawn edge to edge.
fn app_icon() -> egui::IconData {
    let size = 64usize;
    let mut rgba = vec![0u8; size * size * 4];
    let sheet = (255u8, 255u8, 255u8);
    let fold = (222u8, 210u8, 250u8);
    let line = (103u8, 80u8, 164u8);

    let s = size as f32;
    let radius = s * 0.075;
    let left = s * 0.15;
    let top = s * 0.02;
    let right = s * 0.87;
    let bottom = s * 0.98;
    let corner = s * 0.22;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            let inside_sheet = fx >= left
                && fx <= right
                && fy >= top
                && fy <= bottom
                && rounded_rect_contains(
                    fx - left,
                    fy - top,
                    right - left,
                    bottom - top,
                    radius,
                );
            // The diagonal cut removes the top-right corner.
            let cut = fx > right - corner
                && fy < top + corner
                && (fx - (right - corner)) + ((top + corner) - fy) > corner;
            let in_fold = fx > right - corner
                && fx <= right
                && fy >= top
                && fy <= top + corner
                && (fx - (right - corner)) <= (fy - top);

            let mut color = None;
            if inside_sheet && !cut {
                color = Some(if in_fold { fold } else { sheet });
            }

            // Three note lines on the sheet.
            let lines = [(0.24, 0.90), (0.40, 0.60), (0.56, 0.78)];
            for (y_factor, width_factor) in lines {
                let ly = top + (bottom - top) * y_factor;
                let lx0 = left + s * 0.10;
                let lx1 = lx0 + (right - left - s * 0.20) * width_factor;
                if fy >= ly && fy <= ly + s * 0.058 && fx >= lx0 && fx <= lx1 {
                    color = Some(line);
                }
            }

            if let Some((r, g, b)) = color {
                let idx = (y * size + x) * 4;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }
    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

fn rounded_rect_contains(x: f32, y: f32, width: f32, height: f32, radius: f32) -> bool {
    let inner_x_max = width - radius;
    let inner_y_max = height - radius;
    let dx = if x < radius {
        radius - x
    } else if x > inner_x_max {
        x - inner_x_max
    } else {
        0.0
    };
    let dy = if y < radius {
        radius - y
    } else if y > inner_y_max {
        y - inner_y_max
    } else {
        0.0
    };
    dx * dx + dy * dy <= radius * radius
}
