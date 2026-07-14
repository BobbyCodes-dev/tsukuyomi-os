use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub os: String,
    pub tags: String,
    pub notes: String,
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
        "CREATE TABLE IF NOT EXISTS assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            host TEXT NOT NULL,
            os TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_assets_user ON assets(user_id);",
    )?;
    Ok(())
}

pub fn add_asset(
    user_id: i64,
    name: &str,
    host: &str,
    os: &str,
    tags: &str,
    notes: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO assets (user_id, name, host, os, tags, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![user_id, name, host, os, tags, notes],
    )?;
    Ok(())
}

pub fn update_asset(
    user_id: i64,
    id: i64,
    name: &str,
    host: &str,
    os: &str,
    tags: &str,
    notes: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE assets SET name = ?1, host = ?2, os = ?3, tags = ?4, notes = ?5 WHERE id = ?6 AND user_id = ?7",
        params![name, host, os, tags, notes, id, user_id],
    )?;
    Ok(())
}

pub fn delete_asset(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM assets WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_assets(user_id: i64) -> Result<Vec<Asset>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, host, os, tags, notes FROM assets WHERE user_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(Asset {
            id: row.get(0)?,
            name: row.get(1)?,
            host: row.get(2)?,
            os: row.get(3)?,
            tags: row.get(4)?,
            notes: row.get(5)?,
        })
    })?;
    let mut assets = Vec::new();
    for row in rows {
        assets.push(row?);
    }
    Ok(assets)
}
