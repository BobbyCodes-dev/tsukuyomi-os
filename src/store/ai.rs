use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    OpenAiCompatible,
    Gemini,
    Ollama,
    OllamaCloud,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProvider {
    pub id: i64,
    pub kind: ProviderKind,
    pub model: String,
    pub endpoint: String,
    pub vault_label: String,
    pub is_default: bool,
}

impl Default for AiProvider {
    fn default() -> Self {
        Self {
            id: 0,
            kind: ProviderKind::Anthropic,
            model: "claude-3-5-sonnet-20241022".to_string(),
            endpoint: "https://api.anthropic.com/v1/messages".to_string(),
            vault_label: "ai-provider-anthropic".to_string(),
            is_default: true,
        }
    }
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
        "CREATE TABLE IF NOT EXISTS ai_providers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            model TEXT NOT NULL,
            endpoint TEXT NOT NULL,
            vault_label TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    Ok(())
}

pub fn save_provider(user_id: i64, provider: &AiProvider) -> Result<i64> {
    let _ = user_id;
    let conn = open_db()?;
    if provider.id == 0 {
        conn.execute(
            "INSERT INTO ai_providers (kind, model, endpoint, vault_label, is_default)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                serde_json::to_string(&provider.kind)?,
                &provider.model,
                &provider.endpoint,
                &provider.vault_label,
                provider.is_default as i64
            ],
        )?;
        Ok(conn.last_insert_rowid())
    } else {
        conn.execute(
            "UPDATE ai_providers SET kind=?1, model=?2, endpoint=?3, vault_label=?4, is_default=?5
             WHERE id=?6",
            params![
                serde_json::to_string(&provider.kind)?,
                &provider.model,
                &provider.endpoint,
                &provider.vault_label,
                provider.is_default as i64,
                provider.id
            ],
        )?;
        Ok(provider.id)
    }
}

pub fn load_provider(_user_id: i64) -> Result<Option<AiProvider>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, model, endpoint, vault_label, is_default FROM ai_providers
         WHERE is_default = 1 ORDER BY id DESC LIMIT 1"
    )?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let kind_str: String = row.get(1)?;
        let kind: ProviderKind = serde_json::from_str(&kind_str)?;
        Ok(Some(AiProvider {
            id: row.get(0)?,
            kind,
            model: row.get(2)?,
            endpoint: row.get(3)?,
            vault_label: row.get(4)?,
            is_default: row.get::<usize, i64>(5)? != 0,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn list_providers(_user_id: i64) -> Result<Vec<AiProvider>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, kind, model, endpoint, vault_label, is_default FROM ai_providers ORDER BY id"
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let kind_str: String = row.get(1)?;
        let kind: ProviderKind = serde_json::from_str(&kind_str)?;
        out.push(AiProvider {
            id: row.get(0)?,
            kind,
            model: row.get(2)?,
            endpoint: row.get(3)?,
            vault_label: row.get(4)?,
            is_default: row.get::<usize, i64>(5)? != 0,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_roundtrip() {
        let uid = 777;
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-ai-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.clone()));
        let mut p = AiProvider::default();
        p.model = "test-model".to_string();
        save_provider(uid, &p).unwrap();
        let loaded = load_provider(uid).unwrap().expect("provider loaded");
        assert_eq!(loaded.kind, ProviderKind::Anthropic);
        assert_eq!(loaded.model, "test-model");
    }
}
