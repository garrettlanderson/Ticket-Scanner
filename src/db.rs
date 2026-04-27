use chrono::Local;
use rusqlite::{params, Connection, Error, ErrorCode, Result};

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct ScanRecord {
    pub ticket_number: i64,
    pub scanned_at: String,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scans (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_number INTEGER NOT NULL UNIQUE,
                scanned_at    TEXT    NOT NULL
            );",
        )?;
        Ok(Database { conn })
    }

    /// Attempt to insert a new scan.
    /// Returns `Ok(true)` on success, `Ok(false)` if the ticket is a duplicate.
    pub fn try_insert_scan(&self, ticket_number: i64) -> Result<bool> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        match self.conn.execute(
            "INSERT INTO scans (ticket_number, scanned_at) VALUES (?1, ?2)",
            params![ticket_number, now],
        ) {
            Ok(_) => Ok(true),
            Err(Error::SqliteFailure(err, _))
                if err.code == ErrorCode::ConstraintViolation =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    pub fn get_total_count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM scans", [], |row| row.get(0))
    }

    pub fn get_last_n_scans(&self, n: i64) -> Result<Vec<ScanRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT ticket_number, scanned_at FROM scans ORDER BY id DESC LIMIT ?1",
        )?;
        let records = stmt
            .query_map(params![n], |row| {
                Ok(ScanRecord {
                    ticket_number: row.get(0)?,
                    scanned_at: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }

    /// Count scans in the last `minutes` minutes (used for plates/hr estimate).
    pub fn get_recent_count(&self, minutes: i64) -> Result<i64> {
        let since = (Local::now()
            - chrono::Duration::try_minutes(minutes).unwrap_or_default())
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
        self.conn.query_row(
            "SELECT COUNT(*) FROM scans WHERE scanned_at >= ?1",
            params![since],
            |row| row.get(0),
        )
    }
}
