use chrono::Local;
use rusqlite::{params, Connection, Result};

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct ScanRecord {
    pub ticket_number: i64,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// First time this ticket has been seen.
    New,
    /// Ticket exists already; it was NOT inserted again.
    Duplicate,
    /// Ticket exists already, but was inserted again because the duplicate
    /// override was active.
    DuplicateForced,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scans (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_number INTEGER NOT NULL,
                scanned_at    TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_scans_ticket ON scans(ticket_number);
            CREATE INDEX IF NOT EXISTS idx_scans_time   ON scans(scanned_at);",
        )?;

        // Migrate older databases that had `ticket_number INTEGER NOT NULL UNIQUE`
        // — required so the duplicate-override feature can actually insert.
        let needs_migration: bool = {
            let sql: Option<String> = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='scans'",
                    [],
                    |r| r.get(0),
                )
                .ok();
            sql.map(|s| s.to_uppercase().contains("UNIQUE")).unwrap_or(false)
        };
        if needs_migration {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE scans_new (
                    id            INTEGER PRIMARY KEY AUTOINCREMENT,
                    ticket_number INTEGER NOT NULL,
                    scanned_at    TEXT    NOT NULL
                 );
                 INSERT INTO scans_new (id, ticket_number, scanned_at)
                    SELECT id, ticket_number, scanned_at FROM scans;
                 DROP TABLE scans;
                 ALTER TABLE scans_new RENAME TO scans;
                 CREATE INDEX IF NOT EXISTS idx_scans_ticket ON scans(ticket_number);
                 CREATE INDEX IF NOT EXISTS idx_scans_time   ON scans(scanned_at);
                 COMMIT;",
            )?;
        }

        Ok(Database { conn })
    }

    /// Attempt to insert a scan.
    ///
    /// * If the ticket has not been seen before → inserts and returns `New`.
    /// * If the ticket already exists and `force` is false → does NOT insert,
    ///   returns `Duplicate`.
    /// * If the ticket already exists and `force` is true → inserts a second
    ///   row anyway and returns `DuplicateForced`.
    pub fn try_insert_scan(&self, ticket_number: i64, force: bool) -> Result<InsertOutcome> {
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM scans WHERE ticket_number = ?1",
            params![ticket_number],
            |r| r.get(0),
        )?;

        if exists > 0 && !force {
            return Ok(InsertOutcome::Duplicate);
        }

        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute(
            "INSERT INTO scans (ticket_number, scanned_at) VALUES (?1, ?2)",
            params![ticket_number, now],
        )?;

        Ok(if exists > 0 {
            InsertOutcome::DuplicateForced
        } else {
            InsertOutcome::New
        })
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
