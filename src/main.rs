mod app;
mod db;
mod logger;

use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    // Force a 1:1 scale factor on X11 *before* winit reads the environment.
    // The Elecrow / LCD-Show drivers report the panel's true physical size
    // (~153×86 mm at 1024×600), which causes winit to compute a HiDPI scale
    // factor (~1.77×) and inflate the window past the screen.  We want pixel
    // accurate rendering, then control UI size with our own pixels_per_point.
    // Override at launch with `WINIT_X11_SCALE_FACTOR=...` if you really
    // want winit's auto-detected value.
    if std::env::var_os("WINIT_X11_SCALE_FACTOR").is_none() {
        // SAFETY: single-threaded at this point, before any other code runs.
        unsafe {
            std::env::set_var("WINIT_X11_SCALE_FACTOR", "1.0");
        }
    }

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
        .with_inner_size([800.0, 480.0])
        .with_min_inner_size([480.0, 320.0]);

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
        // Don't restore previous window geometry — it can pin the window to
        // a stale size that's larger than the current display.
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Ticket Scanner",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::TicketScannerApp::new(cc, db, logger)))),
    )
}
