use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct VMBackend {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub reason: String,
}

fn windows_edition() -> String {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "(Get-WindowsEdition -Online).Edition"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

fn has_feature(feature: &str) -> bool {
    let cmd = format!("(Get-WindowsOptionalFeature -FeatureName {feature} -Online).State");
    match Command::new("powershell").args(["-NoProfile", "-Command", &cmd]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains("Enabled"),
        Err(_) => false,
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for ext in ["", ".exe", ".cmd", ".bat"] {
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

pub fn detect_backends() -> Vec<VMBackend> {
    let edition = windows_edition();
    let pro_editions = matches!(edition.as_str(), "Professional" | "Enterprise" | "Education");

    let (sandbox_available, sandbox_reason) = if pro_editions {
        if which("WindowsSandbox.exe").is_some() {
            if has_feature("Containers-DisposableClientVM") {
                (true, "Available".to_string())
            } else {
                (false, "Windows Sandbox feature not enabled".to_string())
            }
        } else {
            (false, "Windows Sandbox executable not found".to_string())
        }
    } else {
        (false, format!("Windows {edition} does not include Windows Sandbox"))
    };

    let (hyperv_available, hyperv_reason) = if pro_editions {
        if has_feature("Microsoft-Hyper-V-All") {
            (true, "Available".to_string())
        } else {
            (false, "Hyper-V feature not enabled".to_string())
        }
    } else {
        (false, format!("Windows {edition} does not include Hyper-V"))
    };

    let vbox_available = which("VBoxManage").is_some();
    let vbox_reason = if vbox_available {
        "VirtualBox found"
    } else {
        "VirtualBox not installed (download from virtualbox.org)"
    }
    .to_string();

    let vmware_available = which("vmrun").is_some() || which("vmplayer").is_some();
    let vmware_reason = if vmware_available { "VMware found" } else { "VMware not installed" }.to_string();

    let qemu_available = which("qemu-system-x86_64").is_some();
    let qemu_reason = if qemu_available { "QEMU found" } else { "QEMU not installed" }.to_string();

    vec![
        VMBackend {
            id: "windows_sandbox".to_string(),
            name: "Windows Sandbox".to_string(),
            available: sandbox_available,
            reason: sandbox_reason,
        },
        VMBackend {
            id: "hyperv".to_string(),
            name: "Hyper-V".to_string(),
            available: hyperv_available,
            reason: hyperv_reason,
        },
        VMBackend {
            id: "virtualbox".to_string(),
            name: "VirtualBox".to_string(),
            available: vbox_available,
            reason: vbox_reason,
        },
        VMBackend {
            id: "vmware".to_string(),
            name: "VMware".to_string(),
            available: vmware_available,
            reason: vmware_reason,
        },
        VMBackend {
            id: "qemu".to_string(),
            name: "QEMU/KVM".to_string(),
            available: qemu_available,
            reason: qemu_reason,
        },
    ]
}

pub fn choose_backend(backends: &[VMBackend], prefer: Option<&str>) -> Option<VMBackend> {
    let mut order: Vec<&str> = Vec::new();
    if let Some(p) = prefer {
        order.push(p);
    }
    order.extend(["windows_sandbox", "hyperv", "virtualbox", "vmware", "qemu"]);
    for id in order {
        if let Some(b) = backends.iter().find(|b| b.id == id) {
            if b.available {
                return Some(b.clone());
            }
        }
    }
    None
}

pub fn suggest_action(backends: &[VMBackend]) -> String {
    let edition = windows_edition();
    if let Some(chosen) = choose_backend(backends, None) {
        return format!("Best available backend: {}. Press Enter to launch.", chosen.name);
    }
    if edition == "Home" {
        return "Windows Home detected. Install VirtualBox (virtualbox.org) to run Tsukuyomi OS in a sandboxed VM.".to_string();
    }
    "No VM backend found. Install VirtualBox, VMware, or QEMU, or enable Windows Sandbox/Hyper-V.".to_string()
}
