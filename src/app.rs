use eframe::egui::{
    self, Align, Color32, FontId, Frame, Layout, Margin, RichText, Rounding, Stroke, Vec2,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::db::{Database, InsertOutcome, ScanRecord};
use crate::logger::Logger;

// ── Colour palette ──────────────────────────────────────────────────────────
const BG: Color32 = Color32::from_rgb(18, 18, 34);
const PANEL: Color32 = Color32::from_rgb(28, 30, 52);
const ACCENT: Color32 = Color32::from_rgb(80, 140, 255);
const SUCCESS: Color32 = Color32::from_rgb(0, 200, 120);
const MUTED: Color32 = Color32::from_rgb(140, 145, 175);
const WHITE: Color32 = Color32::WHITE;

// ── Status state ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
enum Status {
    Idle,
    Success(i64),
    Duplicate(i64),
    Error(String),
}

// ── App state ─────────────────────────────────────────────────────────────────
pub struct TicketScannerApp {
    db: Database,
    logger: Arc<Logger>,
    input: String,
    recent: Vec<ScanRecord>,
    total: i64,
    plates_hr: f64,
    status: Status,
    status_at: Option<Instant>,
    last_stats_refresh: Instant,
    /// While `Some` and not yet expired, duplicate scans are accepted.
    allow_dup_until: Option<Instant>,
}

impl TicketScannerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, db: Database, logger: Arc<Logger>) -> Self {
        configure_visuals(&cc.egui_ctx);

        let total = db.get_total_count().unwrap_or(0);
        let recent = db.get_last_n_scans(10).unwrap_or_default();
        let plates_hr = calc_plates_hr(&db);

        logger.log(
            "INFO",
            &format!("loaded: total={total}, recent={}", recent.len()),
        );

        Self {
            db,
            logger,
            input: String::new(),
            recent,
            total,
            plates_hr,
            status: Status::Idle,
            status_at: None,
            last_stats_refresh: Instant::now(),
            allow_dup_until: None,
        }
    }

    fn refresh_stats(&mut self) {
        self.total = self.db.get_total_count().unwrap_or(self.total);
        self.recent = self.db.get_last_n_scans(10).unwrap_or_default();
        self.plates_hr = calc_plates_hr(&self.db);
        self.last_stats_refresh = Instant::now();
    }

    fn dup_override_active(&self) -> bool {
        self.allow_dup_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    fn dup_override_remaining(&self) -> Option<Duration> {
        self.allow_dup_until.and_then(|t| {
            let now = Instant::now();
            if now < t { Some(t - now) } else { None }
        })
    }

    fn submit(&mut self) {
        let raw = self.input.trim().to_owned();
        self.input.clear();
        if raw.is_empty() {
            return;
        }

        // Strip anything that isn't a digit (e.g. prefix characters from some scanners)
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();

        let force = self.dup_override_active();

        match digits.parse::<i64>() {
            Ok(n) => match self.db.try_insert_scan(n, force) {
                Ok(InsertOutcome::New) => {
                    self.logger
                        .log("SCAN", &format!("#{n} accepted (raw={raw:?})"));
                    self.status = Status::Success(n);
                    self.refresh_stats();
                }
                Ok(InsertOutcome::DuplicateForced) => {
                    self.logger.log(
                        "SCAN",
                        &format!("#{n} accepted as DUPLICATE (override active, raw={raw:?})"),
                    );
                    self.status = Status::Success(n);
                    self.refresh_stats();
                }
                Ok(InsertOutcome::Duplicate) => {
                    self.logger
                        .log("DUPLICATE", &format!("#{n} rejected (raw={raw:?})"));
                    self.status = Status::Duplicate(n);
                }
                Err(e) => {
                    self.logger.log("ERROR", &format!("DB error on #{n}: {e}"));
                    self.status = Status::Error(format!("DB error: {e}"));
                }
            },
            Err(_) => {
                self.logger
                    .log("INVALID", &format!("input not a number: {raw:?}"));
                self.status = Status::Error(format!("Not a valid number: {raw}"));
            }
        }

        self.status_at = Some(Instant::now());
    }

    fn enable_dup_override(&mut self) {
        let until = Instant::now() + Duration::from_secs(5 * 60);
        self.allow_dup_until = Some(until);
        self.logger
            .log("OVERRIDE", "duplicate-allow window opened for 5 minutes");
    }

    // ── Number-pad button helper ─────────────────────────────────────────────
    fn pad_btn(ui: &mut egui::Ui, label: &str) -> bool {
        let btn = egui::Button::new(RichText::new(label).size(32.0).strong())
            .min_size(Vec2::new(96.0, 84.0))
            .fill(Color32::from_rgb(38, 40, 70))
            .stroke(Stroke::new(1.0, Color32::from_rgb(70, 75, 120)))
            .rounding(Rounding::same(8.0));
        ui.add(btn).clicked()
    }
    // ── Number-pad button with a custom-drawn icon (no font glyphs needed) ────
    fn pad_icon_btn(ui: &mut egui::Ui, kind: PadIcon) -> bool {
        let size = Vec2::new(96.0, 84.0);
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        // Background (mirrors pad_btn styling, with hover/press feedback)
        let fill = if response.is_pointer_button_down_on() {
            Color32::from_rgb(70, 80, 130)
        } else if response.hovered() {
            Color32::from_rgb(55, 60, 100)
        } else {
            Color32::from_rgb(38, 40, 70)
        };
        let painter = ui.painter();
        painter.rect(
            rect,
            Rounding::same(8.0),
            fill,
            Stroke::new(1.0, Color32::from_rgb(70, 75, 120)),
        );

        match kind {
            PadIcon::Backspace => draw_backspace_icon(painter, rect, WHITE),
            PadIcon::Enter => draw_enter_icon(painter, rect, WHITE),
        }

        response.clicked()
    }
}

// ── Visual configuration ─────────────────────────────────────────────────────
fn configure_visuals(ctx: &egui::Context) {
    let mut vis = egui::Visuals::dark();
    vis.window_fill = BG;
    vis.panel_fill = BG;
    vis.override_text_color = Some(WHITE);
    vis.widgets.inactive.bg_fill = Color32::from_rgb(38, 40, 70);
    vis.widgets.hovered.bg_fill = Color32::from_rgb(55, 60, 100);
    vis.widgets.active.bg_fill = Color32::from_rgb(70, 80, 130);

    let mut style = (*ctx.style()).clone();
    style.visuals = vis;
    style.text_styles = [
        (egui::TextStyle::Heading, FontId::proportional(26.0)),
        (egui::TextStyle::Body, FontId::proportional(18.0)),
        (egui::TextStyle::Button, FontId::proportional(20.0)),
        (egui::TextStyle::Small, FontId::proportional(13.0)),
        (egui::TextStyle::Monospace, FontId::monospace(16.0)),
    ]
    .into();
    ctx.set_style(style);
}

fn calc_plates_hr(db: &Database) -> f64 {
    // count / 10 min × 60 = plates per hour
    db.get_recent_count(10).unwrap_or(0) as f64 * 6.0
}

fn time_of(s: &str) -> String {
    // Stored as "YYYY-MM-DD HH:MM:SS" — reformat as 12-hour with AM/PM.
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        dt.format("%-I:%M:%S %p").to_string()
    } else {
        s.to_string()
    }
}

// ── Custom-drawn pad icons (avoids missing-glyph boxes) ─────────────────────
#[derive(Clone, Copy)]
enum PadIcon {
    Backspace,
    Enter,
}

fn draw_backspace_icon(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    use egui::{Stroke, pos2};
    // Pentagon pointing left + an "x" inside.
    let c = rect.center();
    let w = 44.0;
    let h = 30.0;
    let tip_x = c.x - w * 0.5;
    let right_x = c.x + w * 0.5;
    let top_y = c.y - h * 0.5;
    let bot_y = c.y + h * 0.5;
    let mid_y = c.y;

    let pts = vec![
        pos2(tip_x, mid_y),
        pos2(tip_x + 12.0, top_y),
        pos2(right_x, top_y),
        pos2(right_x, bot_y),
        pos2(tip_x + 12.0, bot_y),
    ];
    painter.add(egui::Shape::convex_polygon(
        pts,
        Color32::TRANSPARENT,
        Stroke::new(3.0, color),
    ));

    // The 'x'
    let x_c = egui::pos2(c.x + 6.0, mid_y);
    let r = 6.0;
    let stroke = Stroke::new(2.5, color);
    painter.line_segment(
        [pos2(x_c.x - r, x_c.y - r), pos2(x_c.x + r, x_c.y + r)],
        stroke,
    );
    painter.line_segment(
        [pos2(x_c.x + r, x_c.y - r), pos2(x_c.x - r, x_c.y + r)],
        stroke,
    );
}

fn draw_enter_icon(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    use egui::{Stroke, pos2};
    // Hooked arrow: a vertical bar dropping then bending left into an arrowhead.
    let c = rect.center();
    let stroke = Stroke::new(3.5, color);

    // top-right corner of the hook
    let top_right = pos2(c.x + 18.0, c.y - 16.0);
    // bottom-right where the bend starts
    let bot_right = pos2(c.x + 18.0, c.y + 8.0);
    // tip of the arrow on the left
    let tip = pos2(c.x - 22.0, c.y + 8.0);

    // Vertical segment
    painter.line_segment([top_right, bot_right], stroke);
    // Horizontal segment
    painter.line_segment([bot_right, tip], stroke);
    // Arrow head (two diagonals)
    painter.line_segment([tip, pos2(tip.x + 10.0, tip.y - 8.0)], stroke);
    painter.line_segment([tip, pos2(tip.x + 10.0, tip.y + 8.0)], stroke);
}

// ── eframe::App impl ─────────────────────────────────────────────────────────
impl eframe::App for TicketScannerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint every 5 s so the clock and rate stay fresh
        ctx.request_repaint_after(Duration::from_secs(5));

        // Auto-dismiss status banners
        if let Some(t) = self.status_at {
            let ttl = match self.status {
                Status::Duplicate(_) => Duration::from_secs(8),
                _ => Duration::from_secs(4),
            };
            if t.elapsed() > ttl {
                self.status = Status::Idle;
                self.status_at = None;
            }
        }

        // Expire the duplicate-override window
        if let Some(t) = self.allow_dup_until {
            if Instant::now() >= t {
                self.allow_dup_until = None;
                self.logger
                    .log("OVERRIDE", "duplicate-allow window expired");
            }
        }

        // Refresh stats every 30 s (catches e.g. external DB changes)
        if self.last_stats_refresh.elapsed() > Duration::from_secs(30) {
            self.refresh_stats();
        }

        // ── Keyboard / scanner input ──────────────────────────────────────────
        let mut appended = String::new();
        let mut enter = false;
        let mut back = false;

        ctx.input(|i| {
            for ev in &i.events {
                match ev {
                    egui::Event::Text(t) => appended.push_str(t),
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        ..
                    } => enter = true,
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        ..
                    } => back = true,
                    _ => {}
                }
            }
        });

        self.input.push_str(&appended);
        if back {
            self.input.pop();
        }
        if enter {
            self.submit();
        }

        // ═══════════════════════════════════════════════════════════════════════
        // Title bar
        // ═══════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::top("title")
            .exact_height(52.0)
            .frame(
                Frame::default()
                    .fill(Color32::from_rgb(22, 22, 46))
                    .inner_margin(Margin::symmetric(18.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("🎫  TICKET SCANNER")
                            .size(28.0)
                            .strong()
                            .color(ACCENT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let now = chrono::Local::now().format("%-I:%M:%S %p").to_string();
                        ui.label(RichText::new(now).size(18.0).color(MUTED));
                    });
                });
            });

        // ═══════════════════════════════════════════════════════════════════════
        // Status banner
        // ═══════════════════════════════════════════════════════════════════════
        let (banner_bg, banner_text, banner_color) = match &self.status {
            Status::Idle => (
                Color32::from_rgb(28, 30, 52),
                "Ready — scan a ticket or enter a number below".to_owned(),
                MUTED,
            ),
            Status::Success(n) => (
                Color32::from_rgb(0, 100, 60),
                format!("✅  ACCEPTED — Ticket #{n}"),
                WHITE,
            ),
            Status::Duplicate(n) => (
                Color32::from_rgb(140, 20, 20),
                format!("⚠️  DUPLICATE! Ticket #{n} has already been scanned!"),
                WHITE,
            ),
            Status::Error(msg) => (Color32::from_rgb(120, 80, 0), format!("⚠️  {msg}"), WHITE),
        };

        egui::TopBottomPanel::top("status")
            .exact_height(64.0)
            .frame(Frame::default().fill(banner_bg))
            .show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(&banner_text)
                            .size(24.0)
                            .strong()
                            .color(banner_color),
                    );
                });
            });

        // ═══════════════════════════════════════════════════════════════════════
        // Bottom panel — input display + number pad
        // ═══════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::bottom("numpad")
            .exact_height(390.0)
            .frame(
                Frame::default()
                    .fill(PANEL)
                    .inner_margin(Margin::same(12.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    // ── Left: current input display + action buttons ───────────
                    let left_size = Vec2::new(340.0, ui.available_height());
                    ui.allocate_ui_with_layout(left_size, Layout::top_down(Align::LEFT), |ui| {
                        ui.label(RichText::new("ENTER TICKET NUMBER").size(13.0).color(MUTED));
                        ui.add_space(4.0);

                        // Fixed-size area for the input display so the buttons
                        // below it always stay visible.
                        let input_border = if self.input.is_empty() { MUTED } else { ACCENT };
                        ui.allocate_ui_with_layout(
                            Vec2::new(320.0, 84.0),
                            Layout::top_down(Align::LEFT),
                            |ui| {
                                Frame::default()
                                    .fill(Color32::from_rgb(18, 18, 34))
                                    .stroke(Stroke::new(2.0, input_border))
                                    .rounding(Rounding::same(8.0))
                                    .inner_margin(Margin::symmetric(12.0, 8.0))
                                    .show(ui, |ui| {
                                        ui.set_min_size(Vec2::new(296.0, 64.0));
                                        ui.set_max_size(Vec2::new(296.0, 64.0));
                                        ui.centered_and_justified(|ui| {
                                            let (disp, col) = if self.input.is_empty() {
                                                ("_ _ _ _".to_owned(), MUTED)
                                            } else {
                                                (self.input.clone(), WHITE)
                                            };
                                            ui.label(
                                                RichText::new(disp)
                                                    .size(42.0)
                                                    .strong()
                                                    .color(col),
                                            );
                                        });
                                    });
                            },
                        );

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [156.0, 64.0],
                                    egui::Button::new(RichText::new("CLEAR").size(22.0).strong())
                                        .fill(Color32::from_rgb(80, 25, 25))
                                        .rounding(Rounding::same(8.0)),
                                )
                                .clicked()
                            {
                                self.input.clear();
                            }
                            ui.add_space(8.0);
                            if ui
                                .add_sized(
                                    [156.0, 64.0],
                                    egui::Button::new(RichText::new("SUBMIT").size(22.0).strong())
                                        .fill(Color32::from_rgb(20, 80, 45))
                                        .rounding(Rounding::same(8.0)),
                                )
                                .clicked()
                            {
                                self.submit();
                            }
                        });

                        // ── Allow-duplicates override toggle ────────────────
                        ui.add_space(10.0);
                        let remaining = self.dup_override_remaining();
                        let (btn_label, btn_fill) = match remaining {
                            Some(d) => {
                                let secs = d.as_secs();
                                (
                                    format!(
                                        "ALLOWING DUPLICATES  {:02}:{:02}",
                                        secs / 60,
                                        secs % 60
                                    ),
                                    Color32::from_rgb(150, 90, 0),
                                )
                            }
                            None => (
                                "ALLOW DUPLICATES (5 min)".to_owned(),
                                Color32::from_rgb(60, 50, 90),
                            ),
                        };
                        if ui
                            .add_sized(
                                [320.0, 52.0],
                                egui::Button::new(RichText::new(btn_label).size(18.0).strong())
                                    .fill(btn_fill)
                                    .rounding(Rounding::same(8.0)),
                            )
                            .clicked()
                        {
                            self.enable_dup_override();
                        }
                    });

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // ── Right: number pad ─────────────────────────────────────
                    let pad_size = Vec2::new(330.0, ui.available_height());
                    ui.allocate_ui_with_layout(pad_size, Layout::top_down(Align::LEFT), |ui| {
                        egui::Grid::new("numpad_grid")
                            .spacing(Vec2::splat(6.0))
                            .show(ui, |ui| {
                                for row in [["7", "8", "9"], ["4", "5", "6"], ["1", "2", "3"]] {
                                    for d in row {
                                        if Self::pad_btn(ui, d) {
                                            self.input.push_str(d);
                                        }
                                    }
                                    ui.end_row();
                                }
                                if Self::pad_btn(ui, "0") {
                                    self.input.push('0');
                                }
                                if Self::pad_icon_btn(ui, PadIcon::Backspace) {
                                    self.input.pop();
                                }
                                if Self::pad_icon_btn(ui, PadIcon::Enter) {
                                    self.submit();
                                }
                                ui.end_row();
                            });
                    });
                });
            });

        // ═══════════════════════════════════════════════════════════════════════
        // Central panel — stats (left) + recent scans (right)
        // ═══════════════════════════════════════════════════════════════════════
        egui::CentralPanel::default()
            .frame(Frame::default().fill(BG).inner_margin(Margin::same(10.0)))
            .show(ctx, |ui| {
                // ── Stat cards: two squares side-by-side ─────────────────────
                ui.horizontal(|ui| {
                    let avail_w = ui.available_width();
                    let card_size = ((avail_w - 12.0) / 2.0).min(220.0).max(160.0);

                    stat_card(
                        ui,
                        "TOTAL SCANNED",
                        &self.total.to_string(),
                        "tickets",
                        ACCENT,
                        card_size,
                    );
                    ui.add_space(12.0);
                    stat_card(
                        ui,
                        "PLATES / HR",
                        &format!("{:.0}", self.plates_hr),
                        "last 10 min",
                        SUCCESS,
                        card_size,
                    );
                });

                ui.add_space(10.0);

                // ── Recent scans list (fills the gap above the numpad) ──────
                Frame::default()
                    .fill(PANEL)
                    .rounding(Rounding::same(12.0))
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width() - 4.0);
                        ui.set_min_height(ui.available_height() - 4.0);
                        ui.label(RichText::new("RECENTSCANs").size(14.0).color(MUTED));
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Column headers
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("TICKET #").size(13.0).strong().color(MUTED));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new("TIME SCANNED")
                                        .size(13.0)
                                        .strong()
                                        .color(MUTED),
                                );
                            });
                        });
                        ui.add_space(2.0);
                        ui.separator();
                        ui.add_space(2.0);

                        if self.recent.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    RichText::new("No tickets scanned yet")
                                        .size(16.0)
                                        .color(MUTED),
                                );
                            });
                        } else {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for (i, scan) in self.recent.iter().enumerate() {
                                    let (color, prefix) = if i == 0 {
                                        (WHITE, "▶ ")
                                    } else {
                                        (MUTED, "   ")
                                    };
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{prefix}#{:>6}",
                                                scan.ticket_number
                                            ))
                                            .size(20.0)
                                            .strong()
                                            .color(color),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(time_of(&scan.scanned_at))
                                                        .size(16.0)
                                                        .color(MUTED),
                                                );
                                            },
                                        );
                                    });
                                    if i < self.recent.len() - 1 {
                                        ui.separator();
                                    }
                                }
                            });
                        }
                    });
            });
    }
}

// ── Shared widget: stat card ──────────────────────────────────────────────────
fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, sub: &str, color: Color32, size: f32) {
    Frame::default()
        .fill(PANEL)
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_min_size(Vec2::new(size, size));
            ui.set_max_size(Vec2::new(size, size));
            ui.vertical_centered(|ui| {
                ui.add_space(size * 0.10);
                ui.label(RichText::new(label).size(15.0).color(MUTED));
                ui.add_space(size * 0.05);
                ui.label(RichText::new(value).size(size * 0.42).strong().color(color));
                ui.add_space(size * 0.02);
                ui.label(RichText::new(sub).size(13.0).color(MUTED));
            });
        });
}
