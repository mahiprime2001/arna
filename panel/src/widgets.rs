//! Small reusable UI pieces built on egui's painter, so the look is ours and
//! doesn't depend on icon fonts.

use crate::theme::Palette;
use eframe::egui::{
    self, Align2, Color32, FontId, Rect, Response, Sense, Stroke, Ui, Vec2,
};

/// A rounded card container.
pub fn card<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::none()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .rounding(12.0)
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, add)
        .inner
}

/// A filled accent button. Returns true when clicked.
pub fn primary_button(ui: &mut Ui, p: &Palette, label: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    let size = Vec2::new(galley.size().x + 32.0, 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if resp.hovered() { p.accent_hover } else { p.accent };
    ui.painter().rect_filled(rect, 9.0, fill);
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        Color32::WHITE,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// An outlined (ghost) button. Returns true when clicked.
pub fn ghost_button(ui: &mut Ui, p: &Palette, label: &str, danger: bool) -> bool {
    let text_col = if danger { p.danger } else { p.text };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(14.0), text_col);
    let size = Vec2::new(galley.size().x + 28.0, 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let stroke_col = if danger {
        p.danger
    } else if resp.hovered() {
        p.accent
    } else {
        p.border
    };
    if resp.hovered() {
        ui.painter().rect_filled(rect, 9.0, p.card_hover);
    }
    ui.painter().rect_stroke(rect, 9.0, Stroke::new(1.0, stroke_col));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, text_col);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// A sidebar navigation entry with optional unread badge. Returns true when clicked.
pub fn nav_item(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    selected: bool,
    badge: usize,
) -> bool {
    let size = Vec2::new(ui.available_width(), 40.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let bg = if selected {
        p.accent_soft
    } else if resp.hovered() {
        p.card
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 9.0, bg);
    if selected {
        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
        ui.painter().rect_filled(bar, 2.0, p.accent);
    }
    let text_col = if selected { p.text } else { p.text_dim };
    ui.painter().text(
        rect.left_center() + Vec2::new(16.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.5),
        text_col,
    );
    if badge > 0 {
        let center = rect.right_center() - Vec2::new(18.0, 0.0);
        ui.painter().circle_filled(center, 9.0, p.accent);
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            badge.to_string(),
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

#[derive(Clone, Copy)]
pub enum WinBtn {
    Min,
    Max,
    Close,
}

/// A window control (minimize/maximize/close) drawn with lines. Returns true when clicked.
pub fn window_button(ui: &mut Ui, p: &Palette, kind: WinBtn) -> bool {
    let size = Vec2::new(44.0, ui.available_height());
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let bg = if hovered {
        if matches!(kind, WinBtn::Close) {
            p.danger
        } else {
            p.card_hover
        }
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0.0, bg);
    let col = if hovered && matches!(kind, WinBtn::Close) {
        Color32::WHITE
    } else {
        p.text_dim
    };
    let stroke = Stroke::new(1.3, col);
    let c = rect.center();
    let r = 5.0;
    match kind {
        WinBtn::Min => {
            ui.painter()
                .line_segment([c - Vec2::new(r, 0.0), c + Vec2::new(r, 0.0)], stroke);
        }
        WinBtn::Max => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, Vec2::splat(2.0 * r)),
                1.0,
                stroke,
            );
        }
        WinBtn::Close => {
            ui.painter()
                .line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(r, -r), c + Vec2::new(-r, r)], stroke);
        }
    }
    resp.clicked()
}

/// A modern toggle switch. Flips `on` when clicked; returns the response.
pub fn toggle(ui: &mut Ui, p: &Palette, on: &mut bool) -> Response {
    let size = Vec2::new(38.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let radius = rect.height() / 2.0;
    let track = if *on { p.accent } else { p.border };
    ui.painter().rect_filled(rect, radius, track);
    let cx = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    ui.painter().circle_filled(
        egui::pos2(cx, rect.center().y),
        radius - 3.0,
        Color32::WHITE,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// A circular avatar showing the person's initial.
pub fn avatar(ui: &mut Ui, p: &Palette, initial: char, diameter: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    ui.painter().circle_filled(rect.center(), diameter / 2.0, p.accent);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initial.to_string(),
        FontId::proportional(diameter * 0.44),
        Color32::WHITE,
    );
}

/// A small status dot (online/offline).
pub fn status_dot(ui: &mut Ui, p: &Palette, online: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    let col = if online { p.good } else { p.text_dim };
    ui.painter().circle_filled(rect.center(), 4.0, col);
}
