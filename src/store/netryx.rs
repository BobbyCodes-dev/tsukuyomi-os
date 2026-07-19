use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A Netryx Astra V2 geolocation result — stores the image path,
/// predicted location, confidence, coordinates, and the full
/// model output JSON for later review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetryxResult {
    pub id: i64,
    pub user_id: i64,
    pub image_path: String,
    pub predicted_location: String,
    pub confidence: String,
    pub latitude: String,
    pub longitude: String,
    pub model_output: String,
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
        "CREATE TABLE IF NOT EXISTS netryx_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            image_path TEXT NOT NULL,
            predicted_location TEXT NOT NULL DEFAULT '',
            confidence TEXT NOT NULL DEFAULT '',
            latitude TEXT NOT NULL DEFAULT '',
            longitude TEXT NOT NULL DEFAULT '',
            model_output TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_netryx_user ON netryx_results(user_id);",
    )?;
    Ok(())
}

pub fn add_result(
    user_id: i64,
    image_path: &str,
    predicted_location: &str,
    confidence: &str,
    latitude: &str,
    longitude: &str,
    model_output: &str,
) -> Result<i64> {
    let conn = open_db()?;
    let now = now_string();
    conn.execute(
        "INSERT INTO netryx_results
         (user_id, image_path, predicted_location, confidence, latitude, longitude, model_output, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![user_id, image_path, predicted_location, confidence, latitude, longitude, model_output, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_results(user_id: i64) -> Result<Vec<NetryxResult>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, user_id, image_path, predicted_location, confidence, latitude, longitude, model_output, created_at
         FROM netryx_results WHERE user_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(NetryxResult {
            id: row.get(0)?,
            user_id: row.get(1)?,
            image_path: row.get(2)?,
            predicted_location: row.get(3)?,
            confidence: row.get(4)?,
            latitude: row.get(5)?,
            longitude: row.get(6)?,
            model_output: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn delete_result(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM netryx_results WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
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
            "tsukuyomi-netryx-test-{}-{}",
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
    fn netryx_result_roundtrip() {
        let dir = temp_env();
        let user_id = 1;
        add_result(user_id, "C:\\images\\test.jpg", "Paris, France", "0.87", "48.8566", "2.3522", "{}").unwrap();
        add_result(user_id, "C:\\images\\test2.jpg", "Tokyo, Japan", "0.92", "35.6762", "139.6503", "{}").unwrap();
        let results = list_results(user_id).unwrap();
        assert_eq!(results.len(), 2);
        // DESC ordering — most recent first
        assert_eq!(results[0].predicted_location, "Tokyo, Japan");
        assert_eq!(results[0].latitude, "35.6762");
        delete_result(user_id, results[0].id).unwrap();
        assert_eq!(list_results(user_id).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}