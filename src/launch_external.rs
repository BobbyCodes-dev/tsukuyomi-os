use std::path::PathBuf;
use std::process::Command;

const BROWSER_URL: &str = "https://duckduckgo.com";

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn open_browser() {
    let _ = Command::new("cmd").args(["/C", "start", "", BROWSER_URL]).spawn();
}

pub fn open_terminal() {
    let _ = Command::new("powershell.exe").spawn();
}

pub fn open_files() {
    if let Some(home) = home_dir() {
        let _ = Command::new("explorer.exe").arg(home).spawn();
    }
}
