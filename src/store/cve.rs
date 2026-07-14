use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CveEntry {
    pub id: i64,
    pub cve_id: String,
    pub description: String,
    pub cvss_score: String,
    pub severity: String,
    pub refs: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NvdResponse {
    #[serde(default)]
    pub vulnerabilities: Vec<NvdVulnerability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NvdVulnerability {
    pub cve: NvdCve,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NvdCve {
    pub id: String,
    #[serde(default)]
    pub descriptions: Vec<NvdDescription>,
    #[serde(default)]
    pub references: Vec<NvdReference>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NvdDescription {
    pub lang: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NvdReference {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct CvssMetric {
    pub base_score: Option<f64>,
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
        "CREATE TABLE IF NOT EXISTS cve_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            cve_id TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            cvss_score TEXT NOT NULL DEFAULT '',
            severity TEXT NOT NULL DEFAULT '',
            refs TEXT NOT NULL DEFAULT '',
            fetched_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_cve_user ON cve_entries(user_id);",
    )?;
    Ok(())
}

pub fn add_cve(
    user_id: i64,
    cve_id: &str,
    description: &str,
    cvss_score: &str,
    severity: &str,
    references: &str,
    fetched_at: &str,
) -> Result<i64> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO cve_entries (user_id, cve_id, description, cvss_score, severity, refs, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(cve_id) DO UPDATE SET
            description = excluded.description,
            cvss_score = excluded.cvss_score,
            severity = excluded.severity,
            refs = excluded.refs,
            fetched_at = excluded.fetched_at",
        params![user_id, cve_id, description, cvss_score, severity, references, fetched_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_cve(user_id: i64, id: i64) -> Result<()> {
    let conn = open_db()?;
    conn.execute(
        "DELETE FROM cve_entries WHERE id = ?1 AND user_id = ?2",
        params![id, user_id],
    )?;
    Ok(())
}

pub fn list_cves(user_id: i64) -> Result<Vec<CveEntry>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, cve_id, description, cvss_score, severity, refs, fetched_at
         FROM cve_entries WHERE user_id = ?1 ORDER BY cve_id",
    )?;
    let rows = stmt.query_map(params![user_id], |row| {
        Ok(CveEntry {
            id: row.get(0)?,
            cve_id: row.get(1)?,
            description: row.get(2)?,
            cvss_score: row.get(3)?,
            severity: row.get(4)?,
            refs: row.get(5)?,
            fetched_at: row.get(6)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[allow(dead_code)]
pub fn lookup_cve_id(user_id: i64, cve_id: &str) -> Result<Option<CveEntry>> {
    Ok(list_cves(user_id)?.into_iter().find(|c| c.cve_id.eq_ignore_ascii_case(cve_id)))
}

pub async fn fetch_nvd(cve_id: &str) -> Result<NvdResponse> {
    let url = format!(
        "https://services.nvd.nist.gov/rest/json/cves/2.0?cveId={}",
        urlencoding::encode(cve_id)
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("NVD returned {}", resp.status());
    }
    Ok(resp.json::<NvdResponse>().await?)
}

pub fn parse_nvd(data: &NvdResponse) -> Option<(&str, String, String, String, String, String)> {
    let v = data.vulnerabilities.first()?;
    let cve = &v.cve;
    let description = cve
        .descriptions
        .iter()
        .find(|d| d.lang == "en")
        .map(|d| d.value.clone())
        .unwrap_or_default();
    let refs = cve
        .references
        .iter()
        .map(|r| r.url.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let cvss_score = "N/A".to_string();
    let severity = "Unknown".to_string();
    Some((&cve.id, description, cvss_score, severity, refs, today()))
}

pub fn upsert_from_nvd(user_id: i64, cve_id: &str, data: &NvdResponse) -> Result<i64> {
    if let Some((id, desc, cvss, sev, refs, fetched_at)) = parse_nvd(data) {
        add_cve(user_id, id, &desc, &cvss, &sev, &refs, &fetched_at)
    } else {
        add_cve(user_id, cve_id, "No NVD data returned.", "", "", "", &today())
    }
}

fn today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let days = now.duration_since(UNIX_EPOCH).unwrap().as_secs() / 86_400;
    let (y, m, d) = unix_days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn unix_days_to_ymd(mut days: u64) -> (i32, u32, u32) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year as u64 { break; }
        days -= days_in_year as u64;
        year += 1;
    }
    let mut month = 1;
    let days_in_months = [31, if is_leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for dim in days_in_months {
        if days < dim as u64 { break; }
        days -= dim as u64;
        month += 1;
    }
    (year, month, days as u32 + 1)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tsukuyomi-cve-test-{}-{}",
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
    fn cve_crud_roundtrip() {
        let dir = temp_env();
        let user_id = 1;
        add_cve(user_id, "CVE-2023-1234", "Test CVE", "9.8", "Critical", "https://nvd.nist.gov/", "2026-07-14").unwrap();
        let items = list_cves(user_id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].cve_id, "CVE-2023-1234");
        add_cve(user_id, "CVE-2023-1234", "Updated", "9.9", "Critical", "", "2026-07-15").unwrap();
        assert_eq!(list_cves(user_id).unwrap()[0].description, "Updated");
        delete_cve(user_id, items[0].id).unwrap();
        assert!(list_cves(user_id).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn nvd_parse() {
        let json = serde_json::json!({
            "vulnerabilities": [{
                "cve": {
                    "id": "CVE-2023-1234",
                    "descriptions": [{"lang": "en", "value": "Sample description"}],
                    "references": [{"url": "https://example.com"}]
                }
            }]
        });
        let resp: NvdResponse = serde_json::from_value(json).unwrap();
        let parsed = parse_nvd(&resp).unwrap();
        assert_eq!(parsed.0, "CVE-2023-1234");
        assert_eq!(parsed.1, "Sample description");
    }
}
