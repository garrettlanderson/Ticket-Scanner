mod app;
mod db;
mod logger;

use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    // Allow overriding paths via environment variables
    let db_path = std::env::var("SCANNER_DB_PATH").unwrap_or_else(|_| "tickets.db".to_string());
    let log_path = std::env::var("SCANNER_LOG_PATH").unwrap_or_else(|_| "tickets.log".to_string());

    let db = db::Database::new(&db_path).expect("Failed to open/create the ticket database");
    let logger =
        Arc::new(logger::Logger::new(&log_path).expect("Failed to open/create the log file"));
    logger.log("INFO", &format!("database: {db_path}"));

    // Window-mode selection.  Wayland compositors (labwc/wayfire on Pi OS) often
    // ignore `with_maximized`, so we offer a real fullscreen fallback.
    //   SCANNER_WINDOW_MODE = fullscreen | maximized | windowed   (default: fullscreen)
    let mode = std::env::var("SCANNER_WINDOW_MODE")
        .unwrap_or_else(|_| "fullscreen".to_string())
        .to_lowercase();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Ticket Scanner")
        .with_inner_size([1024.0, 600.0])
        .with_min_inner_size([640.0, 400.0]);

    match mode.as_str() {
        "windowed" => {}
        "maximized" => {
            viewport = viewport.with_maximized(true);
        }
        // "fullscreen" or anything else
        _ => {
            viewport = viewport.with_fullscreen(true).with_decorations(false);
        }
    }

    logger.log("INFO", &format!("window mode: {mode}"));

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Ticket Scanner",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::TicketScannerApp::new(cc, db, logger)))),
    )
}
