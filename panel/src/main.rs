//! Arna — desktop shell (mock data only). Custom title bar, navigation, and
//! the app views: dashboard, workspaces, notifications, profile, settings.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Mock scaffolding: some fields/helpers exist for views we haven't built yet.
#![allow(dead_code)]

mod app;
mod mock;
mod theme;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([860.0, 560.0])
            .with_decorations(false) // we draw our own title bar
            .with_title("Arna"),
        ..Default::default()
    };
    eframe::run_native("Arna", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
