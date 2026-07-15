use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const OLLAMA_INSTALLER_URL: &str = "https://ollama.com/download/OllamaSetup.exe";

fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for ext in ["", ".exe"] {
        let candidate_name = format!("{name}{ext}");
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(&candidate_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn installed_path() -> Option<PathBuf> {
    if let Some(p) = which("ollama") {
        return Some(p);
    }
    let local_app_data = std::env::var_os("LOCALAPPDATA")?;
    let candidate = PathBuf::from(local_app_data).join("Programs").join("Ollama").join("ollama.exe");
    candidate.is_file().then_some(candidate)
}

pub fn is_reachable() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok()
}

pub enum EnsureResult {
    AlreadyRunning,
    Started,
    StartFailed,
    NotInstalled,
}

pub fn ensure_running() -> EnsureResult {
    if is_reachable() {
        return EnsureResult::AlreadyRunning;
    }
    let Some(exe) = installed_path() else {
        return EnsureResult::NotInstalled;
    };
    let spawned = Command::new(&exe)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if spawned.is_err() {
        return EnsureResult::StartFailed;
    }
    for _ in 0..20 {
        if is_reachable() {
            return EnsureResult::Started;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    EnsureResult::StartFailed
}

pub fn download_and_launch_installer() -> anyhow::Result<PathBuf> {
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(120)).build()?;
    let resp = client.get(OLLAMA_INSTALLER_URL).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("download failed: {}", resp.status());
    }
    let bytes = resp.bytes()?;
    let installer_path = std::env::temp_dir().join("TsukuyomiOllamaSetup.exe");
    std::fs::write(&installer_path, &bytes)?;
    Command::new("cmd").args(["/C", "start", "Ollama Setup", installer_path.to_string_lossy().as_ref()]).spawn()?;
    Ok(installer_path)
}
