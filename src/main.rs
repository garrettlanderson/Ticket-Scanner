mod app;
mod db;

use eframe::egui;

fn main() -> eframe::Result<()> {
    // Allow overriding the database path via environment variable
    let db_path =
        std::env::var("SCANNER_DB_PATH").unwrap_or_else(|_| "tickets.db".to_string());

    let db = db::Database::new(&db_path).expect("Failed to open/create the ticket database");

    let viewport = egui::ViewportBuilder::default()
        .with_title("Ticket Scanner")
        .with_inner_size([800.0, 480.0])
        .with_min_inner_size([640.0, 400.0]);

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Ticket Scanner",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::TicketScannerApp::new(cc, db)))),
    )
}
