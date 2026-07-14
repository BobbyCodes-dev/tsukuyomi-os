use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng as AeadOsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::Result;
use argon2::Argon2;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type VaultKey = [u8; 32];

#[derive(Debug, Clone)]
pub struct VaultEntry {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub password: String,
    pub notes: String,
}

#[derive(Serialize, Deserialize)]
struct VaultSecret {
    password: String,
    notes: String,
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
        "CREATE TABLE IF NOT EXISTS vault_keys (
            user_id INTEGER PRIMARY KEY,
            salt BLOB NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vault_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            username TEXT NOT NULL DEFAULT '',
            nonce BLOB NOT NULL,
            ciphertext BLOB NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_vault_entries_user ON vault_entries(user_id);",
    )?;
    Ok(())
}

fn user_salt(conn: &Connection, user_id: i64) -> Result<Vec<u8>> {
    let existing = conn.query_row(
        "SELECT salt FROM vault_keys WHERE user_id = ?1",
        params![user_id],
        |row| row.get::<_, Vec<u8>>(0),
    );
    match existing {
        Ok(salt) => Ok(salt),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let mut salt = vec![0u8; 16];
            OsRng.fill_bytes(&mut salt);
            conn.execute(
                "INSERT INTO vault_keys (user_id, salt) VALUES (?1, ?2)",
                params![user_id, salt],
            )?;
            Ok(salt)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn derive_key(user_id: i64, password: &str) -> Result<VaultKey> {
    let conn = open_db()?;
    let salt = user_salt(&conn, user_id)?;
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut key)
        .map_err(|e| anyhow::anyhow!("failed to derive vault key: {e}"))?;
    Ok(key)
}

pub fn encrypt_for_key(key: &VaultKey, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut AeadOsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;
    Ok((nonce.to_vec(), ciphertext))
}

pub fn decrypt_for_key(key: &VaultKey, nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decryption failed (wrong key or corrupt data): {e}"))
}

fn encrypt(key: &VaultKey, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    encrypt_for_key(key, plaintext)
}

fn decrypt(key: &VaultKey, nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    decrypt_for_key(key, nonce, ciphertext)
}

fn encrypt_secret(key: &VaultKey, password: &str, notes: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let secret = VaultSecret { password: password.to_string(), notes: notes.to_string() };
    let plaintext = serde_json::to_vec(&secret)?;
    encrypt(key, &plaintext)
}

fn decrypt_secret(key: &VaultKey, nonce: &[u8], ciphertext: &[u8]) -> Result<VaultSecret> {
    let plaintext = decrypt(key, nonce, ciphertext)?;
    Ok(serde_json::from_slice(&plaintext)?)
}

pub fn add_entry(
    user_id: i64,
    key: &VaultKey,
    name: &str,
    username: &str,
    password: &str,
    notes: &str,
) -> Result<()> {
    let (nonce, ciphertext) = encrypt_secret(key, password, notes)?;
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO vault_entries (user_id, name, username, nonce, ciphertext) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, name, username, nonce, ciphertext],
    )?;
    Ok(())
}

pub fn update_entry(
    user_id: i64,
    key: &VaultKey,
    id: i64,
    name: &str,
    username: &str,
    password: &str,
    notes: &str,
) -> Result<()> {
    let (nonce, ciphertext) = encrypt_secret(key, password, notes)?;
    let conn = open_db()?;
    conn.execute(
        "UPDATE vault_entries SET name = ?1, username = ?2, nonce = ?3, ciphertext = ?4 WHERE id = ?5 AND user_id = ?6",
        params![name, username, nonce, ciphertext, id, user_id],
    )?;
    Ok(())
}

pub fn delete_entry(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM vault_entries WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_entries(user_id: i64, key: &VaultKey) -> Result<Vec<VaultEntry>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, username, nonce, ciphertext FROM vault_entries WHERE user_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
        ))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (id, name, username, nonce, ciphertext) = row?;
        let secret = decrypt_secret(key, &nonce, &ciphertext)?;
        entries.push(VaultEntry { id, name, username, password: secret.password, notes: secret.notes });
    }
    Ok(entries)
}

pub fn list_entry_names(user_id: i64) -> Result<Vec<(i64, String)>> {
    let conn = open_db()?;
    let mut stmt =
        conn.prepare("SELECT id, name FROM vault_entries WHERE user_id = ?1 ORDER BY name")?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut names = Vec::new();
    for row in rows {
        names.push(row?);
    }
    Ok(names)
}

pub fn get_password(user_id: i64, key: &VaultKey, id: i64) -> Result<Option<String>> {
    let conn = open_db()?;
    let row = conn.query_row(
        "SELECT nonce, ciphertext FROM vault_entries WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
        |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
    );
    match row {
        Ok((nonce, ciphertext)) => {
            let secret = decrypt_secret(key, &nonce, &ciphertext)?;
            Ok(Some(secret.password))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key: VaultKey = [7u8; 32];
        let (nonce, ciphertext) = encrypt(&key, b"hunter2 super secret").unwrap();
        let plaintext = decrypt(&key, &nonce, &ciphertext).unwrap();
        assert_eq!(plaintext, b"hunter2 super secret");
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let key: VaultKey = [1u8; 32];
        let wrong_key: VaultKey = [2u8; 32];
        let (nonce, ciphertext) = encrypt(&key, b"top secret").unwrap();
        assert!(decrypt(&wrong_key, &nonce, &ciphertext).is_err());
    }

    #[test]
    fn vault_entry_roundtrip_via_sqlite() {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-vault-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("LOCALAPPDATA", &dir);

        let user_id = 1;
        let key_a = derive_key(user_id, "correct horse battery staple").unwrap();
        let key_b = derive_key(user_id, "correct horse battery staple").unwrap();
        assert_eq!(key_a, key_b, "same password + persisted salt must derive the same key");

        let wrong_key = derive_key(2, "different user entirely").unwrap();
        assert_ne!(key_a, wrong_key);

        add_entry(user_id, &key_a, "Router", "admin", "sup3rsecret", "office closet").unwrap();
        let entries = list_entries(user_id, &key_a).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Router");
        assert_eq!(entries[0].password, "sup3rsecret");
        assert_eq!(entries[0].notes, "office closet");

        assert!(list_entries(user_id, &wrong_key).is_err());

        let id = entries[0].id;
        update_entry(user_id, &key_a, id, "Router", "admin", "newpass", "moved").unwrap();
        let updated = list_entries(user_id, &key_a).unwrap();
        assert_eq!(updated[0].password, "newpass");

        delete_entry(user_id, id).unwrap();
        assert!(list_entries(user_id, &key_a).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
