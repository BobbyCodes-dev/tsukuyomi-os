use std::path::PathBuf;
use std::process::Command;

const BROWSER_URL: &str = "https://duckduckgo.com";

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn open_browser() {
    // "start" is a cmd.exe builtin, not an executable, so it must be invoked
    // through cmd. The empty "" argument is the standard workaround for
    // `start` otherwise treating the first quoted argument as a window title.
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
