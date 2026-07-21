use std::path::PathBuf;
use std::process::Command;

const BROWSER_URL: &str = "https://duckduckgo.com";

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

pub fn open_browser() {
    #[cfg(windows)]
    {
        let _ = Command::new("cmd").args(["/C", "start", "", BROWSER_URL]).spawn();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("xdg-open").arg(BROWSER_URL).spawn();
    }
}

pub fn open_terminal() {
    #[cfg(windows)]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", "Tsukuyomi OS - Terminal", "powershell.exe"])
            .spawn();
    }
    #[cfg(unix)]
    {
        // Try common terminal emulators, falling back to $SHELL
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm", "alacritty", "kitty"] {
            if Command::new("which").arg(term).output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new(term)
                    .arg("--")
                    .arg(&shell)
                    .spawn();
                return;
            }
        }
        // Last resort: just spawn the shell directly
        let _ = Command::new(&shell).spawn();
    }
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
        #[cfg(windows)]
        {
            let _ = Command::new("explorer.exe").arg(home).spawn();
        }
        #[cfg(unix)]
        {
            let _ = Command::new("xdg-open").arg(home).spawn();
        }
    }
}

pub fn open_ssh(host: &str, port: u16, username: &str) {
    let target = if username.is_empty() {
        host.to_string()
    } else {
        format!("{username}@{host}")
    };

    #[cfg(windows)]
    {
        let title = format!("Tsukuyomi OS - SSH: {host}");
        let _ = Command::new("cmd")
            .args(["/C", "start", &title, "ssh", &target, "-p", &port.to_string()])
            .spawn();
    }
    #[cfg(unix)]
    {
        // Try to open in a terminal emulator, otherwise just run ssh in the background
        for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm", "alacritty", "kitty"] {
            if Command::new("which").arg(term).output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new(term)
                    .arg("--")
                    .arg("ssh")
                    .arg(&target)
                    .arg("-p")
                    .arg(port.to_string())
                    .spawn();
                return;
            }
        }
        let _ = Command::new("ssh")
            .arg(&target)
            .arg("-p")
            .arg(port.to_string())
            .spawn();
    }
}

pub fn open_rdp(host: &str, port: u16, username: &str, password: Option<&str>) {
    #[cfg(windows)]
    {
        let cred_target = format!("TERMSRV/{host}");
        if let Some(pass) = password {
            if !username.is_empty() {
                let _ = Command::new("cmdkey")
                    .args([
                        &format!("/generic:{cred_target}"),
                        &format!("/user:{username}"),
                        &format!("/pass:{pass}"),
                    ])
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
                    let _ = Command::new("cmdkey")
                        .arg(format!("/delete:{cleanup_target}"))
                        .status();
                    let _ = std::fs::remove_file(&cleanup_path);
                });
            }
            Err(_) => {
                let _ = std::fs::remove_file(&rdp_path);
            }
        }
    }
    #[cfg(unix)]
    {
        // Try FreeRDP (xfreerdp) or Remmina for RDP on Linux
        let target = format!("{host}:{port}");
        let user_arg = if username.is_empty() {
            String::new()
        } else {
            format!("/u:{username}")
        };

        // Try xfreerdp first
        if Command::new("which")
            .arg("xfreerdp")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            let mut args = vec!["/v:".to_string() + &target];
            if !user_arg.is_empty() {
                args.push(user_arg);
            }
            let _ = Command::new("xfreerdp").args(&args).spawn();
            return;
        }

        // Try remmina
        if Command::new("which")
            .arg("remmina")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            let _ = Command::new("remmina").spawn();
            return;
        }

        // No RDP client available — log a message
        eprintln!(
            "No RDP client found. Install freerdp2-x11 (xfreerdp) or remmina to use RDP connections."
        );
    }
}