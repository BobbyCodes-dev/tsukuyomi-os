use anyhow::Result;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;

pub struct UninstallArgs {
    pub keep_vms: bool,
    pub yes: bool,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

fn display_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

pub fn nuke(args: UninstallArgs) -> Result<()> {
    let mut targets: Vec<PathBuf> = vec![crate::store::data_dir()];
    if let Some(home) = home_dir() {
        targets.push(home.join("AppData").join("Local").join("TsukuyomiOS"));
        targets.push(home.join("AppData").join("Roaming").join("TsukuyomiOS"));
    }

    let mut existing: BTreeSet<PathBuf> = BTreeSet::new();
    for t in &targets {
        if t.exists() {
            existing.insert(t.canonicalize().unwrap_or_else(|_| t.clone()));
        }
    }

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
        if p.is_dir() {
            if args.keep_vms {
                let vm_dir = p.join("vm");
                if let Ok(entries) = std::fs::read_dir(p) {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        if entry_path == vm_dir {
                            continue;
                        }
                        if entry_path.is_dir() {
                            let _ = std::fs::remove_dir_all(&entry_path);
                        } else {
                            let _ = std::fs::remove_file(&entry_path);
                        }
                    }
                }
                if vm_dir.exists() {
                    println!("Removed: {} (preserved vm/)", display_path(p));
                } else {
                    let _ = std::fs::remove_dir_all(p);
                    println!("Removed: {}", display_path(p));
                }
                continue;
            }
            let _ = std::fs::remove_dir_all(p);
            println!("Removed: {}", display_path(p));
        } else {
            match std::fs::remove_file(p) {
                Ok(()) => println!("Removed: {}", display_path(p)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    println!("Removed: {}", display_path(p))
                }
                Err(e) => println!("Failed to remove {}: {e}", display_path(p)),
            }
        }
    }

    println!("Tsukuyomi OS has been removed from this machine.");
    Ok(())
}
