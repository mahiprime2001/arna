//! Arna — a single, empty panel window. Nothing in it yet, on purpose.

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Arna"),
        ..Default::default()
    };
    eframe::run_native("Arna", options, Box::new(|_cc| Ok(Box::<Panel>::default())))
}

#[derive(Default)]
struct Panel;

impl eframe::App for Panel {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // A single empty central panel.
        egui::CentralPanel::default().show(ctx, |_ui| {});
    }
}
