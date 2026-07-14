use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BackupJob {
    pub id: i64,
    pub name: String,
    pub source: String,
    pub destination: String,
    pub frequency: String,
    pub last_run: String,
    pub last_status: String,
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
        "CREATE TABLE IF NOT EXISTS backups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            source TEXT NOT NULL,
            destination TEXT NOT NULL,
            frequency TEXT NOT NULL DEFAULT 'manual',
            last_run TEXT NOT NULL DEFAULT '',
            last_status TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_backups_user ON backups(user_id);",
    )?;
    Ok(())
}

pub fn add_backup(
    user_id: i64,
    name: &str,
    source: &str,
    destination: &str,
    frequency: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO backups (user_id, name, source, destination, frequency) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, name, source, destination, frequency],
    )?;
    Ok(())
}

pub fn update_backup(
    user_id: i64,
    id: i64,
    name: &str,
    source: &str,
    destination: &str,
    frequency: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE backups SET name = ?1, source = ?2, destination = ?3, frequency = ?4 WHERE id = ?5 AND user_id = ?6",
        params![name, source, destination, frequency, id, user_id],
    )?;
    Ok(())
}

pub fn delete_backup(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM backups WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn record_run(user_id: i64, id: i64, last_run: &str, last_status: &str) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE backups SET last_run = ?1, last_status = ?2 WHERE id = ?3 AND user_id = ?4",
        params![last_run, last_status, id, user_id],
    )?;
    Ok(())
}

pub fn list_backups(user_id: i64) -> Result<Vec<BackupJob>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, source, destination, frequency, last_run, last_status FROM backups WHERE user_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(BackupJob {
            id: row.get(0)?,
            name: row.get(1)?,
            source: row.get(2)?,
            destination: row.get(3)?,
            frequency: row.get(4)?,
            last_run: row.get(5)?,
            last_status: row.get(6)?,
        })
    })?;
    let mut backups = Vec::new();
    for row in rows {
        backups.push(row?);
    }
    Ok(backups)
}
