use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct OsintNote {
    pub id: i64,
    pub engagement_id: i64,
    pub title: String,
    pub category: String,
    pub content: String,
    pub source_url: String,
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
        "CREATE TABLE IF NOT EXISTS osint_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            engagement_id INTEGER NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            source_url TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_osint_notes_user ON osint_notes(user_id);",
    )?;
    Ok(())
}

pub fn add_note(
    user_id: i64,
    engagement_id: i64,
    title: &str,
    category: &str,
    content: &str,
    source_url: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO osint_notes (user_id, engagement_id, title, category, content, source_url) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![user_id, engagement_id, title, category, content, source_url],
    )?;
    Ok(())
}

pub fn update_note(
    user_id: i64,
    id: i64,
    engagement_id: i64,
    title: &str,
    category: &str,
    content: &str,
    source_url: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE osint_notes SET engagement_id = ?1, title = ?2, category = ?3, content = ?4, source_url = ?5 WHERE id = ?6 AND user_id = ?7",
        params![engagement_id, title, category, content, source_url, id, user_id],
    )?;
    Ok(())
}

pub fn delete_note(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM osint_notes WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_notes(user_id: i64) -> Result<Vec<OsintNote>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, engagement_id, title, category, content, source_url FROM osint_notes WHERE user_id = ?1 ORDER BY title",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(OsintNote {
            id: row.get(0)?,
            engagement_id: row.get(1)?,
            title: row.get(2)?,
            category: row.get(3)?,
            content: row.get(4)?,
            source_url: row.get(5)?,
        })
    })?;
    let mut notes = Vec::new();
    for row in rows {
        notes.push(row?);
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-osint-notes-test-{}-{}",
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
    fn osint_note_crud_roundtrip() {
        let dir = temp_env();
        let user_id = 1;

        add_note(
            user_id,
            42,
            "Employee list",
            "Personnel",
            "Found on LinkedIn: 12 employees in IT.",
            "https://linkedin.com/company/acme",
        )
        .unwrap();

        let entries = list_notes(user_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Employee list");
        assert_eq!(entries[0].category, "Personnel");

        let id = entries[0].id;
        update_note(
            user_id,
            id,
            42,
            "Employee list",
            "Personnel",
            "Updated: 14 employees in IT.\nCFO named on About page.",
            "https://linkedin.com/company/acme",
        )
        .unwrap();
        let updated = list_notes(user_id).unwrap();
        assert!(updated[0].content.contains("CFO named"));

        add_note(2, 7, "Other note", "Infrastructure", "n/a", "").unwrap();
        let user1 = list_notes(1).unwrap();
        assert_eq!(user1.len(), 1);

        delete_note(user_id, id).unwrap();
        assert!(list_notes(user_id).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
