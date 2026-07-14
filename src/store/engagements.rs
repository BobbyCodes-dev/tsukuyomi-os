use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Engagement {
    pub id: i64,
    pub client_name: String,
    pub engagement_type: String,
    pub scope: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
    pub invoice_ref: String,
    pub notes: String,
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
        "CREATE TABLE IF NOT EXISTS engagements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            client_name TEXT NOT NULL,
            engagement_type TEXT NOT NULL DEFAULT '',
            scope TEXT NOT NULL DEFAULT '',
            start_date TEXT NOT NULL DEFAULT '',
            end_date TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'Scheduled',
            invoice_ref TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_engagements_user ON engagements(user_id);",
    )?;
    Ok(())
}

pub fn add_engagement(
    user_id: i64,
    client_name: &str,
    engagement_type: &str,
    scope: &str,
    start_date: &str,
    end_date: &str,
    status: &str,
    invoice_ref: &str,
    notes: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO engagements (user_id, client_name, engagement_type, scope, start_date, end_date, status, invoice_ref, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![user_id, client_name, engagement_type, scope, start_date, end_date, status, invoice_ref, notes],
    )?;
    Ok(())
}

pub fn update_engagement(
    user_id: i64,
    id: i64,
    client_name: &str,
    engagement_type: &str,
    scope: &str,
    start_date: &str,
    end_date: &str,
    status: &str,
    invoice_ref: &str,
    notes: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE engagements SET client_name = ?1, engagement_type = ?2, scope = ?3, start_date = ?4, end_date = ?5, status = ?6, invoice_ref = ?7, notes = ?8 WHERE id = ?9 AND user_id = ?10",
        params![client_name, engagement_type, scope, start_date, end_date, status, invoice_ref, notes, id, user_id],
    )?;
    Ok(())
}

pub fn delete_engagement(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM engagements WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_engagements(user_id: i64) -> Result<Vec<Engagement>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, client_name, engagement_type, scope, start_date, end_date, status, invoice_ref, notes FROM engagements WHERE user_id = ?1 ORDER BY start_date DESC, client_name",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(Engagement {
            id: row.get(0)?,
            client_name: row.get(1)?,
            engagement_type: row.get(2)?,
            scope: row.get(3)?,
            start_date: row.get(4)?,
            end_date: row.get(5)?,
            status: row.get(6)?,
            invoice_ref: row.get(7)?,
            notes: row.get(8)?,
        })
    })?;
    let mut engagements = Vec::new();
    for row in rows {
        engagements.push(row?);
    }
    Ok(engagements)
}

pub fn list_engagement_labels(user_id: i64) -> Result<Vec<(i64, String)>> {
    Ok(list_engagements(user_id)?
        .into_iter()
        .map(|e| (e.id, format!("{} - {}", e.client_name, e.engagement_type)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-engagements-test-{}-{}",
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
    fn engagement_crud_roundtrip() {
        let dir = temp_env();
        let user_id = 1;

        add_engagement(
            user_id,
            "Acme Corp",
            "WiFi Audit",
            "192.168.10.0/24, on-site AP survey",
            "2026-07-01",
            "2026-07-03",
            "Scheduled",
            "BC-2026-014",
            "",
        )
        .unwrap();

        let entries = list_engagements(user_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].client_name, "Acme Corp");
        assert_eq!(entries[0].engagement_type, "WiFi Audit");
        assert_eq!(entries[0].status, "Scheduled");
        assert_eq!(entries[0].invoice_ref, "BC-2026-014");

        let id = entries[0].id;
        update_engagement(
            user_id,
            id,
            "Acme Corp",
            "WiFi Audit",
            "192.168.10.0/24, on-site AP survey",
            "2026-07-01",
            "2026-07-03",
            "Completed",
            "BC-2026-014",
            "",
        )
        .unwrap();

        let updated = list_engagements(user_id).unwrap();
        assert_eq!(updated[0].status, "Completed");

        delete_engagement(user_id, id).unwrap();
        assert!(list_engagements(user_id).unwrap().is_empty());

        add_engagement(1, "Acme Corp", "Network Scan", "10.0.0.0/24", "2026-07-05", "2026-07-05", "Active", "BC-2026-015", "").unwrap();
        add_engagement(2, "Other Client", "Physical Security", "Main office lobby", "2026-07-06", "2026-07-06", "Scheduled", "", "").unwrap();

        let user1 = list_engagements(1).unwrap();
        assert_eq!(user1.len(), 1);
        assert_eq!(user1[0].client_name, "Acme Corp");

        let labels = list_engagement_labels(1).unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].1, "Acme Corp - Network Scan");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
