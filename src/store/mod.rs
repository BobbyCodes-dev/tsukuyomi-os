pub mod ai;
pub mod ai_client;
pub mod ai_tools;
pub mod aimap;
pub mod assets;
pub mod backups;
pub mod connections;
pub mod cve;
pub mod engagements;
pub mod evidence;
pub mod findings;
pub mod netryx;
pub mod reports;
pub mod osint_notes;
pub mod remote_support;
pub mod scan_requests;
pub mod settings;
pub mod users;
pub mod vault;
pub mod voidaccess;
pub mod phoneinfoga;
pub mod fawkes;
pub mod paramspider;
pub mod photon;
pub mod onionshare;
pub mod reconftw;
pub mod canarytokens;
pub mod john;
pub mod hashcat;
pub mod hydra;
pub mod hashid;
pub mod crunch;

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .expect("LOCALAPPDATA environment variable must be set");
        PathBuf::from(local_app_data).join("TsukuyomiOS")
    }
    #[cfg(unix)]
    {
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg_data).join("TsukuyomiOS")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".local").join("share").join("TsukuyomiOS")
        } else {
            PathBuf::from("/tmp").join("TsukuyomiOS")
        }
    }
}

pub fn ensure_data_dir() -> anyhow::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
pub fn set_data_dir_for_tests(dir: &std::path::Path) {
    crate::store::ai::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::engagements::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::findings::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::evidence::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::cve::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::scan_requests::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::osint_notes::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::voidaccess::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::aimap::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::netryx::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::phoneinfoga::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::fawkes::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::paramspider::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::photon::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::onionshare::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::reconftw::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::canarytokens::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::john::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::hashcat::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::hydra::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::hashid::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    crate::store::crunch::TEST_DB_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
}
