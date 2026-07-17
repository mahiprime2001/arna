//! The app shell: state, custom title bar, navigation, and the views.

use crate::mock::{self, Friend, Notification, User, Workspace};
use crate::theme::Theme;
use crate::widgets::{self, WinBtn};
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Dashboard,
    Workspaces,
    Notifications,
    Profile,
    Settings,
}

pub struct App {
    theme: Theme,
    route: Route,
    user: User,
    notifications: Vec<Notification>,
    friends: Vec<Friend>,
    workspaces: Vec<Workspace>,
    // mock settings
    launch_on_startup: bool,
    reduce_motion: bool,
    show_offline_friends: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::Dark;
        theme.apply(&cc.egui_ctx);
        Self {
            theme,
            route: Route::Dashboard,
            user: mock::user(),
            notifications: mock::notifications(),
            friends: mock::friends(),
            workspaces: mock::workspaces(),
            launch_on_startup: true,
            reduce_motion: false,
            show_offline_friends: true,
        }
    }

    fn unread(&self) -> usize {
        self.notifications.iter().filter(|n| !n.read).count()
    }

    fn initial(&self) -> char {
        self.user.name.chars().next().unwrap_or('A').to_ascii_uppercase()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let p = self.theme.palette();

        // ── Custom title bar ────────────────────────────────────────────────
        egui::TopBottomPanel::top("title_bar")
            .exact_height(44.0)
            .frame(egui::Frame::none().fill(p.panel))
            .show(ctx, |ui| self.title_bar(ui, ctx));

        // ── Sidebar navigation ─────────────────────────────────────────────
        egui::SidePanel::left("nav")
            .exact_width(224.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(p.panel).inner_margin(egui::Margin::symmetric(12.0, 14.0)))
            .show(ctx, |ui| self.sidebar(ui));

        // ── Content ─────────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(p.bg).inner_margin(egui::Margin::same(24.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.route {
                    Route::Dashboard => self.view_dashboard(ui),
                    Route::Workspaces => self.view_workspaces(ui),
                    Route::Notifications => self.view_notifications(ui),
                    Route::Profile => self.view_profile(ui),
                    Route::Settings => self.view_settings(ui),
                });
            });
    }
}

impl App {
    // ── Title bar ───────────────────────────────────────────────────────────
    fn title_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let p = self.theme.palette();
        let rect = ui.max_rect();

        // Drag & double-click-to-maximize on the bar background.
        let drag = ui.interact(rect, egui::Id::new("title_drag"), Sense::click_and_drag());
        if drag.drag_started_by(egui::PointerButton::Primary) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let max = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
        }

        ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            // Logo mark + wordmark.
            let (logo, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
            ui.painter().rect_filled(logo, 6.0, p.accent);
            ui.painter().text(
                logo.center(),
                Align2::CENTER_CENTER,
                "A",
                FontId::proportional(13.0),
                Color32::WHITE,
            );
            ui.add_space(8.0);
            ui.label(RichText::new("Arna").strong().color(p.text));

            // Right side: window controls + unread pill.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::window_button(ui, &p, WinBtn::Close) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if widgets::window_button(ui, &p, WinBtn::Max) {
                    let max = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                if widgets::window_button(ui, &p, WinBtn::Min) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
                let unread = self.unread();
                if unread > 0 {
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, &p, &format!("{unread} new"), false) {
                        self.route = Route::Notifications;
                    }
                }
            });
        });
    }

    // ── Sidebar ─────────────────────────────────────────────────────────────
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette();
        ui.add_space(4.0);
        ui.label(RichText::new("MENU").size(11.0).color(p.text_dim).strong());
        ui.add_space(6.0);

        let items = [
            (Route::Dashboard, "Dashboard", 0usize),
            (Route::Workspaces, "Workspaces", 0),
            (Route::Notifications, "Notifications", self.unread()),
            (Route::Profile, "Profile", 0),
            (Route::Settings, "Settings", 0),
        ];
        for (route, label, badge) in items {
            if widgets::nav_item(ui, &p, label, self.route == route, badge) {
                self.route = route;
            }
            ui.add_space(2.0);
        }

        // Pin the user chip to the bottom.
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(4.0);
            let clicked = self.user_chip(ui);
            if clicked {
                self.route = Route::Profile;
            }
        });
    }

    fn user_chip(&self, ui: &mut egui::Ui) -> bool {
        let p = self.theme.palette();
        let size = Vec2::new(ui.available_width(), 52.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 10.0, p.card);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let ac = rect.left_center() + Vec2::new(20.0, 0.0);
        ui.painter().circle_filled(ac, 15.0, p.accent);
        ui.painter().text(
            ac,
            Align2::CENTER_CENTER,
            self.initial().to_string(),
            FontId::proportional(14.0),
            Color32::WHITE,
        );
        ui.painter().text(
            rect.left_top() + Vec2::new(44.0, 12.0),
            Align2::LEFT_TOP,
            &self.user.name,
            FontId::proportional(13.5),
            p.text,
        );
        ui.painter().text(
            rect.left_top() + Vec2::new(44.0, 30.0),
            Align2::LEFT_TOP,
            &self.user.role,
            FontId::proportional(11.5),
            p.text_dim,
        );
        resp.clicked()
    }

    // ── Shared bits ─────────────────────────────────────────────────────────
    fn page_header(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        let p = self.theme.palette();
        ui.label(RichText::new(title).size(26.0).strong().color(p.text));
        if !subtitle.is_empty() {
            ui.add_space(2.0);
            ui.label(RichText::new(subtitle).size(14.0).color(p.text_dim));
        }
        ui.add_space(18.0);
    }

    // ── Dashboard ───────────────────────────────────────────────────────────
    fn view_dashboard(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette();
        let first_name = self.user.name.split(' ').next().unwrap_or("there");
        self.page_header(ui, &format!("Welcome back, {first_name}"), "Here's what's happening on your machine.");

        let online = self.friends.iter().filter(|f| f.online).count();
        let stats = [
            (self.workspaces.len().to_string(), "Active workspaces"),
            (format!("{online}"), "Friends online"),
            (self.unread().to_string(), "Unread alerts"),
        ];
        ui.columns(3, |cols| {
            for (i, (value, label)) in stats.iter().enumerate() {
                widgets::card(&mut cols[i], &p, |ui| {
                    ui.set_min_height(78.0);
                    ui.label(RichText::new(value).size(30.0).strong().color(p.text));
                    ui.label(RichText::new(*label).size(13.0).color(p.text_dim));
                });
            }
        });

        ui.add_space(18.0);
        widgets::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Start a workspace").size(16.0).strong().color(p.text));
                    ui.label(RichText::new("Lend some compute to a friend — they get their own space, you keep your desktop.").color(p.text_dim));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::primary_button(ui, &p, "New workspace") {
                        self.route = Route::Workspaces;
                    }
                });
            });
        });

        ui.add_space(18.0);
        ui.label(RichText::new("Recent activity").size(15.0).strong().color(p.text));
        ui.add_space(8.0);
        widgets::card(ui, &p, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new("Nothing here yet").color(p.text_dim));
                ui.add_space(16.0);
            });
        });
    }

    // ── Workspaces (empty list) ──────────────────────────────────────────────
    fn view_workspaces(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| self.page_header(ui, "Workspaces", "Isolated places you lend to people you invite."));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let _ = widgets::primary_button(ui, &p, "New workspace");
            });
        });

        if self.workspaces.is_empty() {
            widgets::card(ui, &p, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(28.0);
                    // Simple framed-square glyph.
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                    ui.painter().rect_stroke(r, 10.0, Stroke::new(2.0, p.text_dim));
                    ui.painter().line_segment(
                        [r.center() - Vec2::new(9.0, 0.0), r.center() + Vec2::new(9.0, 0.0)],
                        Stroke::new(2.0, p.text_dim),
                    );
                    ui.painter().line_segment(
                        [r.center() - Vec2::new(0.0, 9.0), r.center() + Vec2::new(0.0, 9.0)],
                        Stroke::new(2.0, p.text_dim),
                    );
                    ui.add_space(14.0);
                    ui.label(RichText::new("No workspaces yet").size(16.0).strong().color(p.text));
                    ui.add_space(2.0);
                    ui.label(RichText::new("Create one to lend compute to a friend — they get their own screen and you keep working.").color(p.text_dim));
                    ui.add_space(16.0);
                    let _ = widgets::primary_button(ui, &p, "Create workspace");
                    ui.add_space(28.0);
                });
            });
        }
    }

    // ── Notifications panel ──────────────────────────────────────────────────
    fn view_notifications(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| self.page_header(ui, "Notifications", "Updates from your workspaces and friends."));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                if widgets::ghost_button(ui, &p, "Mark all read", false) {
                    for n in &mut self.notifications {
                        n.read = true;
                    }
                }
            });
        });

        let count = self.notifications.len();
        for i in 0..count {
            let (title, body, time, read) = {
                let n = &self.notifications[i];
                (n.title.clone(), n.body.clone(), n.time.clone(), n.read)
            };
            let clicked = widgets::card(ui, &p, |ui| {
                let resp = ui.horizontal(|ui| {
                    // unread dot
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    if !read {
                        ui.painter().circle_filled(dot.center(), 4.0, p.accent);
                    }
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&title).strong().color(p.text));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(&time).size(12.0).color(p.text_dim));
                            });
                        });
                        ui.label(RichText::new(&body).color(p.text_dim));
                    });
                });
                resp.response.interact(Sense::click()).clicked()
            });
            if clicked {
                self.notifications[i].read = true;
            }
            ui.add_space(8.0);
        }
    }

    // ── Profile ───────────────────────────────────────────────────────────────
    fn view_profile(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette();
        self.page_header(ui, "Profile", "");

        widgets::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                widgets::avatar(ui, &p, self.initial(), 64.0);
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&self.user.name).size(20.0).strong().color(p.text));
                    ui.label(RichText::new(&self.user.email).color(p.text_dim));
                    ui.add_space(6.0);
                    // role badge
                    let label = format!("  {}  ", self.user.role);
                    let galley = ui.painter().layout_no_wrap(label, FontId::proportional(12.0), Color32::WHITE);
                    let (br, _) = ui.allocate_exact_size(Vec2::new(galley.size().x, 22.0), Sense::hover());
                    ui.painter().rect_filled(br, 6.0, p.accent);
                    ui.painter().galley(br.center() - galley.size() / 2.0, galley, Color32::WHITE);
                });
            });
        });

        ui.add_space(14.0);
        widgets::card(ui, &p, |ui| {
            self.info_row(ui, "Display name", &self.user.name.clone());
            ui.separator();
            self.info_row(ui, "Email", &self.user.email.clone());
            ui.separator();
            self.info_row(ui, "Role", &self.user.role.clone());
        });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            let _ = widgets::primary_button(ui, &p, "Edit profile");
            let _ = widgets::ghost_button(ui, &p, "Sign out", true);
        });
    }

    fn info_row(&self, ui: &mut egui::Ui, label: &str, value: &str) {
        let p = self.theme.palette();
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            ui.label(RichText::new(label).color(p.text_dim));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(value).color(p.text));
            });
        });
    }

    // ── Settings ──────────────────────────────────────────────────────────────
    fn view_settings(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette();
        self.page_header(ui, "Settings", "Preferences for this device. Mock only for now.");

        // Appearance
        ui.label(RichText::new("Appearance").size(15.0).strong().color(p.text));
        ui.add_space(8.0);
        widgets::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Theme").strong().color(p.text));
                    ui.label(RichText::new("Switch between dark and light.").color(p.text_dim));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Light on the right, Dark on the left of it.
                    let light_sel = self.theme == Theme::Light;
                    let clicked_light = if light_sel {
                        widgets::primary_button(ui, &p, "Light")
                    } else {
                        widgets::ghost_button(ui, &p, "Light", false)
                    };
                    ui.add_space(6.0);
                    let dark_sel = self.theme == Theme::Dark;
                    let clicked_dark = if dark_sel {
                        widgets::primary_button(ui, &p, "Dark")
                    } else {
                        widgets::ghost_button(ui, &p, "Dark", false)
                    };
                    if clicked_light && !light_sel {
                        self.theme = Theme::Light;
                        self.theme.apply(ui.ctx());
                    }
                    if clicked_dark && !dark_sel {
                        self.theme = Theme::Dark;
                        self.theme.apply(ui.ctx());
                    }
                });
            });
        });

        ui.add_space(16.0);
        ui.label(RichText::new("General").size(15.0).strong().color(p.text));
        ui.add_space(8.0);
        widgets::card(ui, &p, |ui| {
            let mut launch = self.launch_on_startup;
            self.setting_row(ui, "Launch on startup", "Open Arna when this computer starts.", &mut launch);
            self.launch_on_startup = launch;
            ui.separator();
            let mut motion = self.reduce_motion;
            self.setting_row(ui, "Reduce motion", "Minimize animations across the app.", &mut motion);
            self.reduce_motion = motion;
            ui.separator();
            let mut offline = self.show_offline_friends;
            self.setting_row(ui, "Show offline friends", "List friends who aren't online right now.", &mut offline);
            self.show_offline_friends = offline;
        });

        ui.add_space(16.0);
        ui.label(RichText::new("About").size(15.0).strong().color(p.text));
        ui.add_space(8.0);
        widgets::card(ui, &p, |ui| {
            self.info_row(ui, "Version", "0.1.0 (shell)");
            ui.separator();
            self.info_row(ui, "Build", "mock data only");
        });
    }

    fn setting_row(&self, ui: &mut egui::Ui, label: &str, desc: &str, value: &mut bool) {
        let p = self.theme.palette();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(label).strong().color(p.text));
                ui.label(RichText::new(desc).size(12.5).color(p.text_dim));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::toggle(ui, &p, value);
            });
        });
    }
}
