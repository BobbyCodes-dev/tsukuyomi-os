use std::path::PathBuf;
use std::process::Command;

pub const NMAP_MISSING_MESSAGE: &str = "nmap not found on PATH — install with: apt install nmap / dnf install nmap / pacman -S nmap";

pub fn strip_target(input: &str) -> String {
    let mut target = input.trim();
    if let Some(rest) = target.strip_prefix("https://") {
        target = rest;
    } else if let Some(rest) = target.strip_prefix("http://") {
        target = rest;
    }
    target.trim_end_matches('/').to_string()
}

fn which_nmap() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;

    #[cfg(windows)]
    let extensions = ["", ".exe"];
    #[cfg(unix)]
    let extensions = [""];

    for ext in extensions {
        let candidate_name = format!("nmap{ext}");
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(&candidate_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn nmap_available() -> bool {
    which_nmap().is_some()
}

pub fn run_scan(target: &str) -> Result<String, String> {
    if !nmap_available() {
        return Err(NMAP_MISSING_MESSAGE.to_string());
    }
    let stripped = strip_target(target);
    if stripped.is_empty() {
        return Err("Target is required.".to_string());
    }
    let output = Command::new("nmap")
        .args(["-sV", "-T4", &stripped])
        .output()
        .map_err(|e| format!("Failed to run nmap: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(stdout)
    } else if stdout.is_empty() && stderr.is_empty() {
        Err("nmap exited with an error and produced no output.".to_string())
    } else {
        Err(format!("{stdout}\n{stderr}").trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_target_removes_scheme() {
        assert_eq!(strip_target("https://example.com"), "example.com");
        assert_eq!(strip_target("http://192.168.1.1"), "192.168.1.1");
    }

    #[test]
    fn strip_target_removes_trailing_slash() {
        assert_eq!(strip_target("https://example.com/"), "example.com");
        assert_eq!(strip_target("example.com/"), "example.com");
    }

    #[test]
    fn strip_target_trims_whitespace() {
        assert_eq!(strip_target("  https://example.com/  "), "example.com");
    }

    #[test]
    fn strip_target_leaves_bare_target_untouched() {
        assert_eq!(strip_target("example.com"), "example.com");
        assert_eq!(strip_target("10.0.0.5"), "10.0.0.5");
    }
}