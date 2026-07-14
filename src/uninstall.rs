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

/// `Path::canonicalize()` on Windows returns `\\?\`-prefixed verbatim paths.
/// Those work fine for filesystem calls but are noisy to show a user, so this
/// strips the prefix for display only (Python's `Path.resolve()` never added one).
fn display_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

/// Mirrors Python's `uninstall.nuke()`. Independently guesses the standard
/// Windows data locations rather than only trusting `store::data_dir()`, matching
/// the original's defense against the app's own path-construction logic being
/// wrong or having changed across versions.
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
            if args.keep_vms && p.file_name().map(|n| n == "vm").unwrap_or(false) {
                println!("Preserving VM directory: {}", display_path(p));
                continue;
            }
            // Best-effort like Python's `shutil.rmtree(p, ignore_errors=True)`.
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
