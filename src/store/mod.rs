pub mod settings;
pub mod users;

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
