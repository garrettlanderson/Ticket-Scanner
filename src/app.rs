use eframe::egui::{
    self, Align, Color32, FontId, Frame, Layout, Margin, RichText, Rounding, Stroke, Vec2,
};
use std::time::{Duration, Instant};

use crate::db::{Database, ScanRecord};

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
    input: String,
    recent: Vec<ScanRecord>,
    total: i64,
    plates_hr: f64,
    status: Status,
    status_at: Option<Instant>,
    last_stats_refresh: Instant,
}

impl TicketScannerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, db: Database) -> Self {
        configure_visuals(&cc.egui_ctx);

        let total = db.get_total_count().unwrap_or(0);
        let recent = db.get_last_n_scans(10).unwrap_or_default();
        let plates_hr = calc_plates_hr(&db);

        Self {
            db,
            input: String::new(),
            recent,
            total,
            plates_hr,
            status: Status::Idle,
            status_at: None,
            last_stats_refresh: Instant::now(),
        }
    }

    fn refresh_stats(&mut self) {
        self.total = self.db.get_total_count().unwrap_or(self.total);
        self.recent = self.db.get_last_n_scans(10).unwrap_or_default();
        self.plates_hr = calc_plates_hr(&self.db);
        self.last_stats_refresh = Instant::now();
    }

    fn submit(&mut self) {
        let raw = self.input.trim().to_owned();
        self.input.clear();
        if raw.is_empty() {
            return;
        }

        // Strip anything that isn't a digit (e.g. prefix characters from some scanners)
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();

        match digits.parse::<i64>() {
            Ok(n) if n >= 1000 => match self.db.try_insert_scan(n) {
                Ok(true) => {
                    self.status = Status::Success(n);
                    self.refresh_stats();
                }
                Ok(false) => {
                    self.status = Status::Duplicate(n);
                }
                Err(e) => {
                    self.status = Status::Error(format!("DB error: {e}"));
                }
            },
            Ok(n) => {
                self.status = Status::Error(format!("Ticket #{n} is invalid — must be ≥ 1000"));
            }
            Err(_) => {
                self.status = Status::Error(format!("Not a valid number: {raw}"));
            }
        }

        self.status_at = Some(Instant::now());
    }

    // ── Number-pad button helper ─────────────────────────────────────────────
    fn pad_btn(ui: &mut egui::Ui, label: &str) -> bool {
        let btn = egui::Button::new(RichText::new(label).size(24.0).strong())
            .min_size(Vec2::new(74.0, 62.0))
            .fill(Color32::from_rgb(38, 40, 70))
            .stroke(Stroke::new(1.0, Color32::from_rgb(70, 75, 120)))
            .rounding(Rounding::same(8.0));
        ui.add(btn).clicked()
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
        (
            egui::TextStyle::Heading,
            FontId::proportional(26.0),
        ),
        (egui::TextStyle::Body, FontId::proportional(18.0)),
        (egui::TextStyle::Button, FontId::proportional(20.0)),
        (egui::TextStyle::Small, FontId::proportional(13.0)),
        (
            egui::TextStyle::Monospace,
            FontId::monospace(16.0),
        ),
    ]
    .into();
    ctx.set_style(style);
}

fn calc_plates_hr(db: &Database) -> f64 {
    // count / 10 min × 60 = plates per hour
    db.get_recent_count(10).unwrap_or(0) as f64 * 6.0
}

fn time_of(s: &str) -> &str {
    // stored as "YYYY-MM-DD HH:MM:SS" — time is at offset 11
    if s.len() >= 19 {
        &s[11..19]
    } else {
        s
    }
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
                        let now = chrono::Local::now().format("%H:%M:%S").to_string();
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
            Status::Error(msg) => (
                Color32::from_rgb(120, 80, 0),
                format!("⚠️  {msg}"),
                WHITE,
            ),
        };

        egui::TopBottomPanel::top("status")
            .exact_height(64.0)
            .frame(Frame::default().fill(banner_bg))
            .show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new(&banner_text).size(24.0).strong().color(banner_color));
                });
            });

        // ═══════════════════════════════════════════════════════════════════════
        // Bottom panel — input display + number pad
        // ═══════════════════════════════════════════════════════════════════════
        egui::TopBottomPanel::bottom("numpad")
            .min_height(205.0)
            .frame(Frame::default().fill(PANEL).inner_margin(Margin::same(10.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // ── Left: current input display + action buttons ───────────
                    ui.vertical(|ui| {
                        ui.set_min_width(290.0);
                        ui.label(RichText::new("ENTER TICKET NUMBER").size(13.0).color(MUTED));
                        ui.add_space(4.0);

                        let input_border = if self.input.is_empty() { MUTED } else { ACCENT };
                        Frame::default()
                            .fill(Color32::from_rgb(18, 18, 34))
                            .stroke(Stroke::new(2.0, input_border))
                            .rounding(Rounding::same(8.0))
                            .inner_margin(Margin::symmetric(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.set_min_size(Vec2::new(270.0, 56.0));
                                ui.centered_and_justified(|ui| {
                                    let (disp, col) = if self.input.is_empty() {
                                        ("_ _ _ _".to_owned(), MUTED)
                                    } else {
                                        (self.input.clone(), WHITE)
                                    };
                                    ui.label(RichText::new(disp).size(38.0).strong().color(col));
                                });
                            });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [132.0, 48.0],
                                    egui::Button::new(
                                        RichText::new("✕  Clear").size(18.0),
                                    )
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
                                    [132.0, 48.0],
                                    egui::Button::new(
                                        RichText::new("✓  Submit").size(18.0).strong(),
                                    )
                                    .fill(Color32::from_rgb(20, 80, 45))
                                    .rounding(Rounding::same(8.0)),
                                )
                                .clicked()
                            {
                                self.submit();
                            }
                        });
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // ── Right: number pad ─────────────────────────────────────
                    ui.vertical(|ui| {
                        egui::Grid::new("numpad_grid")
                            .spacing(Vec2::splat(5.0))
                            .show(ui, |ui| {
                                for row in [["7", "8", "9"], ["4", "5", "6"], ["1", "2", "3"]] {
                                    for d in row {
                                        if Self::pad_btn(ui, d) {
                                            self.input.push_str(d);
                                        }
                                    }
                                    ui.end_row();
                                }
                                // Row 4: 0  ⌫  ↵
                                if Self::pad_btn(ui, "0") {
                                    self.input.push('0');
                                }
                                if Self::pad_btn(ui, "⌫") {
                                    self.input.pop();
                                }
                                if Self::pad_btn(ui, "↵") {
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
                ui.horizontal(|ui| {
                    // ── Stat cards ────────────────────────────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(200.0);

                        stat_card(
                            ui,
                            "TOTAL SCANNED",
                            &self.total.to_string(),
                            "tickets",
                            ACCENT,
                        );
                        ui.add_space(10.0);
                        stat_card(
                            ui,
                            "PLATES / HR",
                            &format!("{:.0}", self.plates_hr),
                            "last 10 min",
                            SUCCESS,
                        );
                    });

                    ui.add_space(10.0);

                    // ── Recent scans list ─────────────────────────────────────
                    Frame::default()
                        .fill(PANEL)
                        .rounding(Rounding::same(12.0))
                        .inner_margin(Margin::same(14.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width() - 4.0);
                            ui.label(
                                RichText::new("LAST 10 SCANNED")
                                    .size(13.0)
                                    .color(MUTED),
                            );
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);

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
                                                .size(18.0)
                                                .strong()
                                                .color(color),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(time_of(&scan.scanned_at))
                                                            .size(14.0)
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
            });
    }
}

// ── Shared widget: stat card ──────────────────────────────────────────────────
fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, sub: &str, color: Color32) {
    Frame::default()
        .fill(PANEL)
        .rounding(Rounding::same(12.0))
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_min_size(Vec2::new(188.0, 100.0));
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(label).size(13.0).color(MUTED));
                ui.add_space(2.0);
                ui.label(RichText::new(value).size(48.0).strong().color(color));
                ui.label(RichText::new(sub).size(12.0).color(MUTED));
            });
        });
}
