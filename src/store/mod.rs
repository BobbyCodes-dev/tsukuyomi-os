pub mod assets;
pub mod backups;
pub mod connections;
pub mod cve;
pub mod engagements;
pub mod evidence;
pub mod findings;
pub mod reports;
pub mod osint_notes;
pub mod remote_support;
pub mod scan_requests;
pub mod settings;
pub mod users;
pub mod vault;

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let local_app_data =
        std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA environment variable must be set");
    PathBuf::from(local_app_data).join("bobbycodes").join("TsukuyomiOS")
}

pub fn ensure_data_dir() -> anyhow::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
pub fn set_data_dir_for_tests(dir: &std::path::Path) {
    crate::store::engagements::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::findings::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::evidence::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::cve::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::scan_requests::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::osint_notes::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
}
