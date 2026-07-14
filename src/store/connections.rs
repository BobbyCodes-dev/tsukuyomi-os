use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Ssh,
    Rdp,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Ssh => "ssh",
            Protocol::Rdp => "rdp",
        }
    }

    pub fn from_str(s: &str) -> Protocol {
        match s {
            "rdp" => Protocol::Rdp,
            _ => Protocol::Ssh,
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            Protocol::Ssh => 22,
            Protocol::Rdp => 3389,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SavedConnection {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub protocol: Protocol,
    pub username: String,
    pub vault_entry_id: Option<i64>,
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
        "CREATE TABLE IF NOT EXISTS connections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            host TEXT NOT NULL,
            port INTEGER NOT NULL,
            protocol TEXT NOT NULL,
            username TEXT NOT NULL DEFAULT '',
            vault_entry_id INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_connections_user ON connections(user_id);",
    )?;
    Ok(())
}

pub fn add_connection(
    user_id: i64,
    name: &str,
    host: &str,
    port: u16,
    protocol: Protocol,
    username: &str,
    vault_entry_id: Option<i64>,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO connections (user_id, name, host, port, protocol, username, vault_entry_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![user_id, name, host, port, protocol.as_str(), username, vault_entry_id],
    )?;
    Ok(())
}

pub fn update_connection(
    user_id: i64,
    id: i64,
    name: &str,
    host: &str,
    port: u16,
    protocol: Protocol,
    username: &str,
    vault_entry_id: Option<i64>,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE connections SET name = ?1, host = ?2, port = ?3, protocol = ?4, username = ?5, vault_entry_id = ?6 WHERE id = ?7 AND user_id = ?8",
        params![name, host, port, protocol.as_str(), username, vault_entry_id, id, user_id],
    )?;
    Ok(())
}

pub fn delete_connection(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM connections WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_connections(user_id: i64) -> Result<Vec<SavedConnection>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, protocol, username, vault_entry_id FROM connections WHERE user_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(SavedConnection {
            id: row.get(0)?,
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get::<_, i64>(3)? as u16,
            protocol: Protocol::from_str(&row.get::<_, String>(4)?),
            username: row.get(5)?,
            vault_entry_id: row.get(6)?,
        })
    })?;
    let mut connections = Vec::new();
    for row in rows {
        connections.push(row?);
    }
    Ok(connections)
}
