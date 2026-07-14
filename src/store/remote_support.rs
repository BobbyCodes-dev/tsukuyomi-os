use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub id: i64,
    pub label: String,
    pub remote_id: String,
}

fn db_path() -> Result<PathBuf> {
    Ok(super::ensure_data_dir()?.join("users.db"))
}

fn open_db() -> Result<Connection> {
    let conn = Connection::open(db_path()?)?;
    ensure_schema(&conn)?;
    Ok(conn)
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS remote_support_bookmarks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            label TEXT NOT NULL,
            remote_id TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_remote_support_bookmarks_user ON remote_support_bookmarks(user_id);",
    )?;
    Ok(())
}

pub fn add_bookmark(user_id: i64, label: &str, remote_id: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO remote_support_bookmarks (user_id, label, remote_id) VALUES (?1, ?2, ?3)",
        params![user_id, label, remote_id],
    )?;
    Ok(())
}

pub fn update_bookmark(user_id: i64, id: i64, label: &str, remote_id: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE remote_support_bookmarks SET label = ?1, remote_id = ?2 WHERE id = ?3 AND user_id = ?4",
        params![label, remote_id, id, user_id],
    )?;
    Ok(())
}

pub fn delete_bookmark(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM remote_support_bookmarks WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_bookmarks(user_id: i64) -> Result<Vec<Bookmark>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, label, remote_id FROM remote_support_bookmarks WHERE user_id = ?1 ORDER BY label",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(Bookmark { id: row.get(0)?, label: row.get(1)?, remote_id: row.get(2)? })
    })?;
    let mut bookmarks = Vec::new();
    for row in rows {
        bookmarks.push(row?);
    }
    Ok(bookmarks)
}
