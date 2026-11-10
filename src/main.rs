//! OP-SE — an open-source, Rust-native AI code editor.
//!
//! Ctrl+K edits code in place, Ctrl+L chats about the code (with @file, @folder
//! and @codebase mentions), Ctrl+I runs the agent that edits many files at once,
//! and the codebase is indexed locally so you can ask where things are.

mod agents_view;
mod ai;
mod app;
mod chat_view;
mod chatgpt;
mod diff;
mod fonts;
mod highlight;
mod history;
mod index;
mod settings;
mod theme;
mod view;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    let mut startup = app::Startup {
        open: None,
        screenshot: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screenshot" => startup.screenshot = args.next().map(PathBuf::from),
            "--open" => startup.open = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!("op-se [FOLDER] [--open FOLDER] [--screenshot FILE.png]");
                return Ok(());
            }
            other if !other.starts_with("--") => startup.open = Some(PathBuf::from(other)),
            _ => {}
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0])
        .with_title("OP-SE");
    if startup.screenshot.is_some() {
        // A check must never steal focus from what the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    if startup.screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "OP-SE",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, startup)))),
    )
}
