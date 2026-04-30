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

    let viewport = egui::ViewportBuilder::default()
        .with_title("Ticket Scanner")
        .with_inner_size([800.0, 480.0])
        .with_min_inner_size([640.0, 400.0])
        .with_maximized(true);

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
