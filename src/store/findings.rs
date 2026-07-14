use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Finding {
    pub id: i64,
    pub engagement_id: i64,
    pub title: String,
    pub severity: String,
    pub status: String,
    pub cvss: String,
    pub description: String,
    pub remediation: String,
    pub affected_assets: String,
    pub evidence_ids: String,
    pub cve_ids: String,
    pub reported_at: String,
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
        "CREATE TABLE IF NOT EXISTS findings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            engagement_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            severity TEXT NOT NULL DEFAULT 'Info',
            status TEXT NOT NULL DEFAULT 'Open',
            cvss TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            remediation TEXT NOT NULL DEFAULT '',
            affected_assets TEXT NOT NULL DEFAULT '',
            evidence_ids TEXT NOT NULL DEFAULT '',
            cve_ids TEXT NOT NULL DEFAULT '',
            reported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_findings_user ON findings(user_id);
        CREATE INDEX IF NOT EXISTS idx_findings_engagement ON findings(engagement_id);",
    )?;
    Ok(())
}

pub fn add_finding(
    user_id: i64,
    engagement_id: i64,
    title: &str,
    severity: &str,
    status: &str,
    cvss: &str,
    description: &str,
    remediation: &str,
    affected_assets: &str,
    evidence_ids: &str,
    cve_ids: &str,
    reported_at: &str,
) -> Result<i64> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO findings (user_id, engagement_id, title, severity, status, cvss, description, remediation, affected_assets, evidence_ids, cve_ids, reported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            user_id,
            engagement_id,
            title,
            severity,
            status,
            cvss,
            description,
            remediation,
            affected_assets,
            evidence_ids,
            cve_ids,
            reported_at
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_finding(
    user_id: i64,
    id: i64,
    engagement_id: i64,
    title: &str,
    severity: &str,
    status: &str,
    cvss: &str,
    description: &str,
    remediation: &str,
    affected_assets: &str,
    evidence_ids: &str,
    cve_ids: &str,
    reported_at: &str,
) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "UPDATE findings SET
            engagement_id = ?1,
            title = ?2,
            severity = ?3,
            status = ?4,
            cvss = ?5,
            description = ?6,
            remediation = ?7,
            affected_assets = ?8,
            evidence_ids = ?9,
            cve_ids = ?10,
            reported_at = ?11
         WHERE id = ?12 AND user_id = ?13",
        params![
            engagement_id,
            title,
            severity,
            status,
            cvss,
            description,
            remediation,
            affected_assets,
            evidence_ids,
            cve_ids,
            reported_at,
            id,
            user_id
        ],
    )?;
    Ok(())
}

pub fn delete_finding(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM findings WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_findings(user_id: i64) -> Result<Vec<Finding>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, engagement_id, title, severity, status, cvss, description, remediation, affected_assets, evidence_ids, cve_ids, reported_at
         FROM findings WHERE user_id = ?1 ORDER BY severity DESC, title",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(Finding {
            id: row.get(0)?,
            engagement_id: row.get(1)?,
            title: row.get(2)?,
            severity: row.get(3)?,
            status: row.get(4)?,
            cvss: row.get(5)?,
            description: row.get(6)?,
            remediation: row.get(7)?,
            affected_assets: row.get(8)?,
            evidence_ids: row.get(9)?,
            cve_ids: row.get(10)?,
            reported_at: row.get(11)?,
        })
    })?;
    let mut findings = Vec::new();
    for row in rows {
        findings.push(row?);
    }
    Ok(findings)
}

pub fn list_findings_for_engagement(user_id: i64, engagement_id: i64) -> Result<Vec<Finding>> {
    Ok(list_findings(user_id)?
        .into_iter()
        .filter(|f| f.engagement_id == engagement_id)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-findings-test-{}-{}",
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
    fn finding_crud_roundtrip() {
        let dir = temp_env();
        let user_id = 1;

        add_finding(
            user_id,
            42,
            "Open SSH on WAN",
            "Critical",
            "Open",
            "9.8",
            "Port 22/tcp is exposed to the internet with password auth enabled.",
            "Disable password auth; restrict to VPN only.",
            "192.168.1.10",
            "1,2",
            "CVE-2023-1234",
            "2026-07-14",
        )
        .unwrap();

        let entries = list_findings(user_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Open SSH on WAN");
        assert_eq!(entries[0].severity, "Critical");

        let id = entries[0].id;
        update_finding(
            user_id,
            id,
            42,
            "Open SSH on WAN",
            "Critical",
            "Resolved",
            "9.8",
            "Port 22/tcp is exposed to the internet with password auth enabled.",
            "Disabled password auth; restricted to VPN only.",
            "192.168.1.10",
            "1,2",
            "CVE-2023-1234",
            "2026-07-14",
        )
        .unwrap();

        let updated = list_findings(user_id).unwrap();
        assert_eq!(updated[0].status, "Resolved");

        add_finding(2, 7, "Other finding", "Info", "Open", "", "", "", "", "", "", "").unwrap();
        assert_eq!(list_findings(user_id).unwrap().len(), 1);
        assert_eq!(list_findings_for_engagement(user_id, 42).unwrap().len(), 1);
        assert!(list_findings_for_engagement(user_id, 99).unwrap().is_empty());

        delete_finding(user_id, id).unwrap();
        assert!(list_findings(user_id).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
