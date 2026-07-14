use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;

const LATEST_RELEASE_API: &str = "https://api.github.com/repos/rustdesk/rustdesk/releases/latest";

#[derive(Debug)]
pub enum DownloadEvent {
    Status(String),
    Done(PathBuf),
    Error(String),
}

#[derive(serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

pub fn exe_path() -> PathBuf {
    crate::store::data_dir().join("tools").join("rustdesk.exe")
}

pub fn is_installed() -> bool {
    exe_path().is_file()
}

fn powershell_fetch_text(url: &str) -> Result<String> {
    let ps = format!(
        "$ProgressPreference='SilentlyContinue'; (Invoke-WebRequest -UseBasicParsing -Uri '{url}' -Headers @{{'User-Agent'='TsukuyomiOS'}}).Content"
    );
    let output = Command::new("powershell").args(["-NoProfile", "-Command", &ps]).output()?;
    if !output.status.success() {
        bail!("Failed to fetch {url}: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn powershell_download_file(url: &str, dest: &Path) -> Result<()> {
    let dest_str = dest.to_string_lossy().to_string();
    let ps = format!(
        "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '{url}' -Headers @{{'User-Agent'='TsukuyomiOS'}} -OutFile '{dest_str}'"
    );
    let status = Command::new("powershell").args(["-NoProfile", "-Command", &ps]).status()?;
    if !status.success() {
        bail!("Failed to download {url}");
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn find_portable_asset(release: &GhRelease) -> Result<&GhAsset> {
    release
        .assets
        .iter()
        .find(|a| {
            let n = a.name.to_lowercase();
            n.ends_with(".exe") && n.contains("x86_64") && !n.contains("sciter")
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not find a portable x86_64 Windows .exe asset in RustDesk release {}",
                release.tag_name
            )
        })
}

pub fn ensure_rustdesk(tx: Sender<DownloadEvent>) -> Result<PathBuf> {
    let path = exe_path();
    if path.is_file() {
        let _ = tx.send(DownloadEvent::Status(format!("Using existing RustDesk client at {}", path.display())));
        return Ok(path);
    }

    let dir = path.parent().ok_or_else(|| anyhow!("Could not determine the tools directory."))?.to_path_buf();
    std::fs::create_dir_all(&dir)?;

    let _ = tx.send(DownloadEvent::Status("Querying latest RustDesk release from GitHub...".to_string()));
    let json_text = powershell_fetch_text(LATEST_RELEASE_API)?;
    let release: GhRelease =
        serde_json::from_str(&json_text).map_err(|e| anyhow!("Could not parse GitHub release metadata: {e}"))?;
    let asset = find_portable_asset(&release)?;

    let _ = tx.send(DownloadEvent::Status(format!("Downloading {} ({})...", asset.name, release.tag_name)));
    let tmp_path = dir.join(format!("{}.download", asset.name));
    powershell_download_file(&asset.browser_download_url, &tmp_path)?;

    match asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        Some(expected) => {
            let _ = tx.send(DownloadEvent::Status("Verifying SHA256 (GitHub-published asset digest)...".to_string()));
            let actual = sha256_file(&tmp_path)?;
            if !actual.eq_ignore_ascii_case(expected) {
                let _ = std::fs::remove_file(&tmp_path);
                bail!(
                    "SHA256 mismatch for {}: expected {expected}, got {actual}. Downloaded file was deleted.",
                    asset.name
                );
            }
            let _ = tx.send(DownloadEvent::Status("Checksum verified.".to_string()));
        }
        None => {
            let _ = tx.send(DownloadEvent::Status(
                "Warning: no checksum was published for this RustDesk asset. Proceeding without verification."
                    .to_string(),
            ));
        }
    }

    std::fs::rename(&tmp_path, &path)?;
    let _ = tx.send(DownloadEvent::Status(format!("RustDesk {} ready at {}", release.tag_name, path.display())));
    Ok(path)
}

pub fn launch_host() -> Result<()> {
    let path = exe_path();
    if !path.is_file() {
        bail!("RustDesk client is not installed yet.");
    }
    Command::new(&path).spawn()?;
    Ok(())
}

pub fn launch_connect(remote_id: &str) -> Result<()> {
    let path = exe_path();
    if !path.is_file() {
        bail!("RustDesk client is not installed yet.");
    }
    Command::new(&path).arg("--connect").arg(remote_id).spawn()?;
    Ok(())
}
