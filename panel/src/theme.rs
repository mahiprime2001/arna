//! Theme system — a small palette + a function that stamps it onto egui.

use eframe::egui::{self, Color32, FontId, Stroke, TextStyle, Visuals};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
    pub good: Color32,
}

impl Theme {
    pub fn palette(self) -> Palette {
        match self {
            Theme::Dark => Palette {
                bg: Color32::from_rgb(0x0f, 0x11, 0x17),
                panel: Color32::from_rgb(0x14, 0x17, 0x20),
                card: Color32::from_rgb(0x1b, 0x1f, 0x2a),
                card_hover: Color32::from_rgb(0x22, 0x27, 0x34),
                border: Color32::from_rgb(0x2a, 0x2f, 0x3d),
                text: Color32::from_rgb(0xe6, 0xe8, 0xee),
                text_dim: Color32::from_rgb(0x89, 0x90, 0xa1),
                accent: Color32::from_rgb(0x6d, 0x5e, 0xfc),
                accent_hover: Color32::from_rgb(0x83, 0x76, 0xff),
                accent_soft: Color32::from_rgba_unmultiplied(0x6d, 0x5e, 0xfc, 40),
                danger: Color32::from_rgb(0xe8, 0x3a, 0x3a),
                good: Color32::from_rgb(0x35, 0xc7, 0x59),
            },
            Theme::Light => Palette {
                bg: Color32::from_rgb(0xf5, 0xf6, 0xfa),
                panel: Color32::from_rgb(0xff, 0xff, 0xff),
                card: Color32::from_rgb(0xff, 0xff, 0xff),
                card_hover: Color32::from_rgb(0xf0, 0xf1, 0xf6),
                border: Color32::from_rgb(0xe4, 0xe7, 0xef),
                text: Color32::from_rgb(0x1a, 0x1d, 0x26),
                text_dim: Color32::from_rgb(0x6b, 0x72, 0x84),
                accent: Color32::from_rgb(0x6d, 0x5e, 0xfc),
                accent_hover: Color32::from_rgb(0x59, 0x4b, 0xe8),
                accent_soft: Color32::from_rgba_unmultiplied(0x6d, 0x5e, 0xfc, 30),
                danger: Color32::from_rgb(0xdc, 0x2e, 0x2e),
                good: Color32::from_rgb(0x1f, 0xa8, 0x4a),
            },
        }
    }

    /// Apply this theme's colors, spacing, and type scale to the whole context.
    pub fn apply(self, ctx: &egui::Context) {
        let p = self.palette();
        let mut v = match self {
            Theme::Dark => Visuals::dark(),
            Theme::Light => Visuals::light(),
        };
        v.override_text_color = Some(p.text);
        v.panel_fill = p.bg;
        v.window_fill = p.panel;
        v.extreme_bg_color = p.bg;
        v.faint_bg_color = p.card;
        v.selection.bg_fill = p.accent_soft;
        v.selection.stroke = Stroke::new(1.0, p.accent);
        v.hyperlink_color = p.accent;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.inactive.bg_fill = p.card;
        v.widgets.inactive.weak_bg_fill = p.card;
        v.widgets.hovered.bg_fill = p.card_hover;
        v.widgets.hovered.weak_bg_fill = p.card_hover;
        v.widgets.active.bg_fill = p.accent;
        v.widgets.active.weak_bg_fill = p.accent;
        ctx.set_visuals(v);

        ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(8.0, 8.0);
            s.spacing.button_padding = egui::vec2(12.0, 7.0);
            s.spacing.window_margin = egui::Margin::same(0.0);
            s.text_styles = [
                (TextStyle::Heading, FontId::proportional(22.0)),
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Small, FontId::proportional(12.0)),
                (TextStyle::Monospace, FontId::monospace(13.0)),
            ]
            .into();
        });
    }
}
