use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// Append-only text log used as a backup of every event the app handles.
pub struct Logger {
    file: Mutex<File>,
    #[allow(dead_code)]
    path: String,
}

impl Logger {
    pub fn new(path: &str) -> std::io::Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let logger = Logger {
            file: Mutex::new(file),
            path: path.to_string(),
        };
        logger.log("STARTUP", "ticket scanner started");
        Ok(logger)
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn log(&self, kind: &str, msg: &str) {
        let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let line = format!("[{stamp}] {kind:<10} {msg}\n");
        if let Ok(mut f) = self.file.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
}
