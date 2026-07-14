use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub id: i64,
    pub engagement_id: i64,
    pub target: String,
    pub submitted_at: String,
    pub notes: String,
}

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_DB_DIR: std::cell::RefCell<Option<PathBuf>> = std::cell::RefCell::new(None);
}

fn db_path() -> Result<PathBuf> {
    #[cfg(test)]
    {
        if let Some(dir) = TEST_DB_DIR.with(|d| d.borrow().clone()) {
            std::fs::create_dir_all(&dir)?;
            return Ok(dir.join("users.db"));
        }
    }
    Ok(super::ensure_data_dir()?.join("users.db"))
}

fn open_db() -> Result<Connection> {
    let conn = Connection::open(db_path()?)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS scan_requests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            engagement_id INTEGER NOT NULL,
            target TEXT NOT NULL DEFAULT '',
            submitted_at TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_scan_requests_user ON scan_requests(user_id);",
    )?;
    Ok(())
}

pub fn add_scan_request(user_id: i64, engagement_id: i64, target: &str, submitted_at: &str) -> Result<i64> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO scan_requests (user_id, engagement_id, target, submitted_at, notes) VALUES (?1, ?2, ?3, ?4, '')",
        params![user_id, engagement_id, target, submitted_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_notes(user_id: i64, id: i64, notes: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE scan_requests SET notes = ?1 WHERE id = ?2 AND user_id = ?3",
        params![notes, id, user_id],
    )?;
    Ok(())
}

pub fn delete_scan_request(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM scan_requests WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_scan_requests(user_id: i64) -> Result<Vec<ScanRequest>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, engagement_id, target, submitted_at, notes FROM scan_requests WHERE user_id = ?1 ORDER BY submitted_at DESC, id DESC",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(ScanRequest {
            id: row.get(0)?,
            engagement_id: row.get(1)?,
            target: row.get(2)?,
            submitted_at: row.get(3)?,
            notes: row.get(4)?,
        })
    })?;
    let mut requests = Vec::new();
    for row in rows {
        requests.push(row?);
    }
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-scan-requests-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.clone()));
        dir
    }

    #[test]
    fn scan_request_crud_roundtrip() {
        let dir = temp_env();
        let user_id = 1;

        add_scan_request(user_id, 42, "example.com", "2026-07-14 10:00:00 UTC").unwrap();

        let entries = list_scan_requests(user_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].engagement_id, 42);
        assert_eq!(entries[0].target, "example.com");
        assert_eq!(entries[0].notes, "");

        let id = entries[0].id;
        update_notes(user_id, id, "nmap result: 2 open ports.").unwrap();
        let updated = list_scan_requests(user_id).unwrap();
        assert_eq!(updated[0].notes, "nmap result: 2 open ports.");

        add_scan_request(2, 7, "10.0.0.5", "2026-07-14 11:00:00 UTC").unwrap();
        let user1 = list_scan_requests(1).unwrap();
        assert_eq!(user1.len(), 1);

        delete_scan_request(user_id, id).unwrap();
        assert!(list_scan_requests(user_id).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
