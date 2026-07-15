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
    let _ = Command::new("cmd")
        .args(["/C", "start", "Tsukuyomi OS - Terminal", "powershell.exe"])
        .spawn();
}

pub fn open_ai_agent_window() {
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.display().to_string();
        let _ = Command::new("cmd")
            .args(["/C", "start", "Tsukuyomi OS - AI Agent", &exe_str, "ai-agent"])
            .spawn();
    }
}

pub fn open_files() {
    if let Some(home) = home_dir() {
        let _ = Command::new("explorer.exe").arg(home).spawn();
    }
}

pub fn open_ssh(host: &str, port: u16, username: &str) {
    let target = if username.is_empty() { host.to_string() } else { format!("{username}@{host}") };
    let title = format!("Tsukuyomi OS - SSH: {host}");
    let _ = Command::new("cmd")
        .args(["/C", "start", &title, "ssh", &target, "-p", &port.to_string()])
        .spawn();
}

pub fn open_rdp(host: &str, port: u16, username: &str, password: Option<&str>) {
    let cred_target = format!("TERMSRV/{host}");
    if let Some(pass) = password {
        if !username.is_empty() {
            let _ = Command::new("cmdkey")
                .args([&format!("/generic:{cred_target}"), &format!("/user:{username}"), &format!("/pass:{pass}")])
                .status();
        }
    }

    let mut rdp_path = std::env::temp_dir();
    rdp_path.push(format!("tsukuyomi-{host}-{port}.rdp"));
    let contents = format!(
        "full address:s:{host}:{port}\nusername:s:{username}\nprompt for credentials:i:0\n"
    );
    if std::fs::write(&rdp_path, contents).is_err() {
        return;
    }

    let child = Command::new("cmd")
        .args(["/C", "start", "/wait", "", "mstsc.exe"])
        .arg(&rdp_path)
        .spawn();

    match child {
        Ok(mut child) => {
            let cleanup_target = cred_target;
            let cleanup_path = rdp_path;
            std::thread::spawn(move || {
                let _ = child.wait();
                let _ = Command::new("cmdkey").arg(format!("/delete:{cleanup_target}")).status();
                let _ = std::fs::remove_file(&cleanup_path);
            });
        }
        Err(_) => {
            let _ = std::fs::remove_file(&rdp_path);
        }
    }
}
