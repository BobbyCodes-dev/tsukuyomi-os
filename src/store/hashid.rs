use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single Hashid result row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashidResult {
    pub id: i64,
    pub user_id: i64,
    pub query: String,
    pub step: String,
    pub finding: String,
    pub source_url: String,
    pub created_at: String,
}

/// A saved Hashid query session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashidSession {
    pub id: i64,
    pub user_id: i64,
    pub query: String,
    pub status: String,
    pub result_count: i64,
    pub created_at: String,
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
        "CREATE TABLE IF NOT EXISTS hashid_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            query TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            result_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_hid_session_user ON hashid_sessions(user_id);

        CREATE TABLE IF NOT EXISTS hashid_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            query TEXT NOT NULL,
            step TEXT NOT NULL,
            finding TEXT NOT NULL,
            source_url TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_hid_result_session ON hashid_results(session_id);
        CREATE INDEX IF NOT EXISTS idx_hid_result_user ON hashid_results(user_id);",
    )?;
    Ok(())
}

pub fn create_session(user_id: i64, query: &str) -> Result<i64> {
    let conn = open_db()?;
    let now = now_string();
    conn.execute(
        "INSERT INTO hashid_sessions (user_id, query, status, result_count, created_at)
         VALUES (?1, ?2, 'running', 0, ?3)",
        params![user_id, query, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish_session(user_id: i64, session_id: i64, result_count: i64, status: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE hashid_sessions SET status = ?1, result_count = ?2
         WHERE id = ?3 AND user_id = ?4",
        params![status, result_count, session_id, user_id],
    )?;
    Ok(())
}

pub fn add_result(
    user_id: i64,
    session_id: i64,
    query: &str,
    step: &str,
    finding: &str,
    source_url: &str,
) -> Result<i64> {
    let conn = open_db()?;
    let now = now_string();
    conn.execute(
        "INSERT INTO hashid_results (session_id, user_id, query, step, finding, source_url, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![session_id, user_id, query, step, finding, source_url, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_sessions(user_id: i64) -> Result<Vec<HashidSession>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, user_id, query, status, result_count, created_at
         FROM hashid_sessions WHERE user_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(HashidSession {
            id: row.get(0)?,
            user_id: row.get(1)?,
            query: row.get(2)?,
            status: row.get(3)?,
            result_count: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn list_results(user_id: i64, session_id: i64) -> Result<Vec<HashidResult>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, user_id, query, step, finding, source_url, created_at
         FROM hashid_results WHERE user_id = ?1 AND session_id = ?2 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![user_id, session_id], |row| {
        Ok(HashidResult {
            id: row.get(0)?,
            user_id: row.get(1)?,
            query: row.get(2)?,
            step: row.get(3)?,
            finding: row.get(4)?,
            source_url: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn delete_session(user_id: i64, session_id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM hashid_results WHERE session_id = ?1 AND user_id = ?2",
        params![session_id, user_id],
    )?;
    conn.execute(
        "DELETE FROM hashid_sessions WHERE id = ?1 AND user_id = ?2",
        params![session_id, user_id],
    )?;
    Ok(())
}

fn now_string() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-hashid-test-{}-{}",
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
    fn hashid_session_roundtrip() {
        let dir = temp_env();
        let user_id = 1;
        let sid = create_session(user_id, "5f4dcc3b5aa765d61d8327deb882cf99").unwrap();
        add_result(user_id, sid, "5f4dcc3b5aa765d61d8327deb882cf99", "crack", "password123", "").unwrap();
        add_result(user_id, sid, "5f4dcc3b5aa765d61d8327deb882cf99", "crack", "letmein", "").unwrap();
        let sessions = list_sessions(user_id).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].query, "5f4dcc3b5aa765d61d8327deb882cf99");
        let results = list_results(user_id, sid).unwrap();
        assert_eq!(results.len(), 2);
        finish_session(user_id, sid, 2, "complete").unwrap();
        let sessions = list_sessions(user_id).unwrap();
        assert_eq!(sessions[0].status, "complete");
        assert_eq!(sessions[0].result_count, 2);
        delete_session(user_id, sid).unwrap();
        assert!(list_sessions(user_id).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
