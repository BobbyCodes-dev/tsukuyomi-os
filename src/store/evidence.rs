use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EvidenceItem {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub content: String,
}

pub type VaultKey = super::vault::VaultKey;

#[derive(Serialize, Deserialize)]
struct EvidenceSecret {
    description: String,
    content: String,
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
        "CREATE TABLE IF NOT EXISTS evidence_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            nonce BLOB NOT NULL,
            ciphertext BLOB NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_evidence_user ON evidence_items(user_id);",
    )?;
    Ok(())
}

pub fn add_evidence(
    user_id: i64,
    key: &VaultKey,
    name: &str,
    description: &str,
    content: &str,
) -> Result<i64> {
    let plaintext = serde_json::to_vec(&EvidenceSecret {
        description: description.to_string(),
        content: content.to_string(),
    })?;
    let (nonce, ciphertext) = super::vault::encrypt_for_key(key, &plaintext)?;
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO evidence_items (user_id, name, nonce, ciphertext) VALUES (?1, ?2, ?3, ?4)",
        params![user_id, name, nonce, ciphertext],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_evidence(
    user_id: i64,
    key: &VaultKey,
    id: i64,
    name: &str,
    description: &str,
    content: &str,
) -> Result<()> {
    let plaintext = serde_json::to_vec(&EvidenceSecret {
        description: description.to_string(),
        content: content.to_string(),
    })?;
    let (nonce, ciphertext) = super::vault::encrypt_for_key(key, &plaintext)?;
    let conn = open_db()?;
    conn.execute(
        "UPDATE evidence_items SET name = ?1, nonce = ?2, ciphertext = ?3 WHERE id = ?4 AND user_id = ?5",
        params![name, nonce, ciphertext, id, user_id],
    )?;
    Ok(())
}

pub fn delete_evidence(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM evidence_items WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_evidence(user_id: i64, key: &VaultKey) -> Result<Vec<EvidenceItem>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, nonce, ciphertext FROM evidence_items WHERE user_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Vec<u8>>(2)?, row.get::<_, Vec<u8>>(3)?))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (id, name, nonce, ciphertext) = row?;
        let plaintext = super::vault::decrypt_for_key(key, &nonce, &ciphertext)?;
        let secret: EvidenceSecret = serde_json::from_slice(&plaintext)?;
        items.push(EvidenceItem {
            id,
            name,
            description: secret.description,
            content: secret.content,
        });
    }
    Ok(items)
}

#[allow(dead_code)]
pub fn list_evidence_labels(user_id: i64) -> Result<Vec<(i64, String)>> {
    let conn = open_db()?;
    let mut stmt =
        conn.prepare("SELECT id, name FROM evidence_items WHERE user_id = ?1 ORDER BY name")?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut names = Vec::new();
    for row in rows {
        names.push(row?);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::vault;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-evidence-test-{}-{}",
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
    fn evidence_crud_roundtrip() {
        let dir = temp_env();
        std::env::set_var("LOCALAPPDATA", &dir);
        let user_id = 1;
        let key = vault::derive_key(user_id, "secret password").unwrap();

        add_evidence(user_id, &key, "nmap screenshot", "LAN scan result", "Raw nmap text...").unwrap();
        let items = list_evidence(user_id, &key).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "nmap screenshot");
        assert_eq!(items[0].content, "Raw nmap text...");

        let id = items[0].id;
        update_evidence(user_id, &key, id, "nmap screenshot", "LAN scan result updated", "Updated text.").unwrap();
        let updated = list_evidence(user_id, &key).unwrap();
        assert_eq!(updated[0].description, "LAN scan result updated");

        add_evidence(2, &key, "Other evidence", "x", "y").unwrap();
        assert_eq!(list_evidence(user_id, &key).unwrap().len(), 1);

        delete_evidence(user_id, id).unwrap();
        assert!(list_evidence(user_id, &key).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
