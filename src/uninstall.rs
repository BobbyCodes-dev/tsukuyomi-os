use anyhow::Result;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;

pub struct UninstallArgs {
    pub keep_vms: bool,
    pub yes: bool,
}

#[cfg(windows)]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

#[cfg(unix)]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn display_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    #[cfg(windows)]
    { s.strip_prefix(r"\\?\").unwrap_or(&s).to_string() }
    #[cfg(unix)]
    { s }
}

fn discover_targets() -> BTreeSet<PathBuf> {
    let mut targets: Vec<PathBuf> = vec![crate::store::data_dir()];

    #[cfg(windows)]
    {
        if let Some(home) = home_dir() {
            targets.push(home.join("AppData").join("Local").join("TsukuyomiOS"));
            targets.push(home.join("AppData").join("Roaming").join("TsukuyomiOS"));
        }
    }

    #[cfg(unix)]
    {
        if let Some(home) = home_dir() {
            targets.push(home.join(".local").join("share").join("TsukuyomiOS"));
            targets.push(home.join(".config").join("TsukuyomiOS"));
            targets.push(home.join(".cache").join("TsukuyomiOS"));
        }
    }

    let mut existing: BTreeSet<PathBuf> = BTreeSet::new();
    for t in &targets {
        if t.exists() {
            existing.insert(t.canonicalize().unwrap_or_else(|_| t.clone()));
        }
    }
    existing
}

fn is_exe(p: &std::path::Path) -> bool {
    p.extension().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false)
}

fn remove_data_preserving_exe(p: &std::path::Path, keep_vms: bool) -> Vec<String> {
    let mut messages = Vec::new();
    if p.is_dir() {
        let vm_dir = p.join("vm");
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if is_exe(&entry_path) {
                    continue;
                }
                if keep_vms && entry_path == vm_dir {
                    continue;
                }
                if entry_path.is_dir() {
                    let _ = std::fs::remove_dir_all(&entry_path);
                } else {
                    let _ = std::fs::remove_file(&entry_path);
                }
            }
        }
        let suffix = if keep_vms { " (preserved vm/ and any .exe)" } else { " (preserved any .exe)" };
        messages.push(format!("Removed data from: {}{}", display_path(p), suffix));
    } else if is_exe(p) {
        messages.push(format!("Preserved: {}", display_path(p)));
    } else {
        match std::fs::remove_file(p) {
            Ok(()) => messages.push(format!("Removed: {}", display_path(p))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                messages.push(format!("Removed: {}", display_path(p)))
            }
            Err(e) => messages.push(format!("Failed to remove {}: {e}", display_path(p))),
        }
    }
    messages
}

pub fn nuke(args: UninstallArgs) -> Result<()> {
    let existing = discover_targets();

    if existing.is_empty() {
        println!("Tsukuyomi OS data not found. Nothing to remove.");
        return Ok(());
    }

    println!("This will PERMANENTLY delete the following Tsukuyomi OS data:");
    for p in &existing {
        println!("  {}", display_path(p));
    }

    if !args.yes {
        print!("Type 'NUKE' to confirm: ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if answer.trim() != "NUKE" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    for p in &existing {
        for line in remove_data_preserving_exe(p, args.keep_vms) {
            println!("{line}");
        }
    }

    println!("Tsukuyomi OS has been removed from this machine.");
    Ok(())
}

pub fn nuke_data(keep_vms: bool) -> Vec<String> {
    let existing = discover_targets();
    if existing.is_empty() {
        return vec!["Tsukuyomi OS data not found. Nothing to remove.".to_string()];
    }
    let mut messages = Vec::new();
    for p in &existing {
        messages.extend(remove_data_preserving_exe(p, keep_vms));
    }
    messages.push("Tsukuyomi OS data has been erased.".to_string());
    messages
}
