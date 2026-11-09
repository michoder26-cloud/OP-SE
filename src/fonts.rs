//! System fonts: Segoe UI for the interface, Consolas for code, and a Thai
//! fallback so Thai prompts and answers display correctly.

use std::{fs, sync::Arc};

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut load = |name: &str, paths: &[&str]| -> bool {
        for path in paths {
            if let Ok(bytes) = fs::read(path) {
                fonts
                    .font_data
                    .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                return true;
            }
        }
        false
    };
    let ui = load(
        "ui",
        &[
            r"C:\Windows\Fonts\segoeui.ttf",
            "/System/Library/Fonts/SFNS.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ],
    );
    let code = load(
        "code",
        &[
            r"C:\Windows\Fonts\consola.ttf",
            "/System/Library/Fonts/Menlo.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        ],
    );
    let thai = load(
        "thai",
        &[
            r"C:\Windows\Fonts\LeelawUI.ttf",
            r"C:\Windows\Fonts\tahoma.ttf",
            "/System/Library/Fonts/Supplemental/Thonburi.ttc",
            "/usr/share/fonts/truetype/tlwg/Garuda.ttf",
        ],
    );
    if ui {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "ui".into());
    }
    if code {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "code".into());
    }
    if thai {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("thai".into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("thai".into());
    }
    ctx.set_fonts(fonts);
}
