use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An AIMap scan result — a discovered exposed AI endpoint.
/// Each row stores the IP, port, service name, banner/metadata,
/// and the Shodan-style metadata blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AimapResult {
    pub id: i64,
    pub user_id: i64,
    #[allow(dead_code)]
    pub session_id: i64,
    pub ip: String,
    pub port: i64,
    pub service: String,
    pub banner: String,
    pub metadata: String,
    pub created_at: String,
}

/// A saved AIMap scan session — tracks the query (e.g. "product:ollama"),
/// the Shodan API key used, status, and result count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AimapSession {
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
        "CREATE TABLE IF NOT EXISTS aimap_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            query TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            result_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_aimap_session_user ON aimap_sessions(user_id);

        CREATE TABLE IF NOT EXISTS aimap_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            ip TEXT NOT NULL,
            port INTEGER NOT NULL,
            service TEXT NOT NULL DEFAULT '',
            banner TEXT NOT NULL DEFAULT '',
            metadata TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_aimap_result_session ON aimap_results(session_id);
        CREATE INDEX IF NOT EXISTS idx_aimap_result_user ON aimap_results(user_id);",
    )?;
    Ok(())
}

pub fn create_session(user_id: i64, query: &str) -> Result<i64> {
    let conn = open_db()?;
    let now = now_string();
    conn.execute(
        "INSERT INTO aimap_sessions (user_id, query, status, result_count, created_at)
         VALUES (?1, ?2, 'running', 0, ?3)",
        params![user_id, query, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish_session(user_id: i64, session_id: i64, result_count: i64, status: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE aimap_sessions SET status = ?1, result_count = ?2
         WHERE id = ?3 AND user_id = ?4",
        params![status, result_count, session_id, user_id],
    )?;
    Ok(())
}

pub fn add_result(
    user_id: i64,
    session_id: i64,
    ip: &str,
    port: i64,
    service: &str,
    banner: &str,
    metadata: &str,
) -> Result<i64> {
    let conn = open_db()?;
    let now = now_string();
    conn.execute(
        "INSERT INTO aimap_results (session_id, user_id, ip, port, service, banner, metadata, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![session_id, user_id, ip, port, service, banner, metadata, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_sessions(user_id: i64) -> Result<Vec<AimapSession>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, user_id, query, status, result_count, created_at
         FROM aimap_sessions WHERE user_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(AimapSession {
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

pub fn list_results(user_id: i64, session_id: i64) -> Result<Vec<AimapResult>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, user_id, session_id, ip, port, service, banner, metadata, created_at
         FROM aimap_results WHERE user_id = ?1 AND session_id = ?2 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![user_id, session_id], |row| {
        Ok(AimapResult {
            id: row.get(0)?,
            user_id: row.get(1)?,
            session_id: row.get(2)?,
            ip: row.get(3)?,
            port: row.get(4)?,
            service: row.get(5)?,
            banner: row.get(6)?,
            metadata: row.get(7)?,
            created_at: row.get(8)?,
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
        "DELETE FROM aimap_results WHERE session_id = ?1 AND user_id = ?2",
        params![session_id, user_id],
    )?;
    conn.execute(
        "DELETE FROM aimap_sessions WHERE id = ?1 AND user_id = ?2",
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
            "tsukuyomi-aimap-test-{}-{}",
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
    fn aimap_session_roundtrip() {
        let dir = temp_env();
        let user_id = 1;
        let sid = create_session(user_id, "product:ollama port:11434").unwrap();
        add_result(user_id, sid, "203.0.113.5", 11434, "Ollama API", "Ollama v0.1.0", "{}").unwrap();
        add_result(user_id, sid, "198.51.100.10", 8080, "OpenAI-compatible", "LM Studio Server", "{}").unwrap();
        let sessions = list_sessions(user_id).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].query, "product:ollama port:11434");
        let results = list_results(user_id, sid).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].ip, "203.0.113.5");
        assert_eq!(results[0].port, 11434);
        finish_session(user_id, sid, 2, "complete").unwrap();
        let sessions = list_sessions(user_id).unwrap();
        assert_eq!(sessions[0].status, "complete");
        delete_session(user_id, sid).unwrap();
        assert!(list_sessions(user_id).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}