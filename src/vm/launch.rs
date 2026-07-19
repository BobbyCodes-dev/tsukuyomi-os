use anyhow::{bail, Result};
use std::path::Path;
use std::process::{Command, Stdio};

// ── Windows Sandbox ─────────────────────────────────────────────

#[cfg(windows)]
pub fn launch_windows_sandbox(mapped_folder: Option<&Path>) -> Result<()> {
    let wsb_path = crate::store::ensure_data_dir()?.join("tsukuyomi.wsb");
    let mapped = match mapped_folder {
        Some(folder) if folder.exists() => format!(
            "\n    <MappedFolder>\n      <HostFolder>{}</HostFolder>\n      <ReadOnly>false</ReadOnly>\n    </MappedFolder>",
            folder.display()
        ),
        _ => String::new(),
    };
    let content = format!(
        "<Configuration>\n  <vGPU>Enable</vGPU>\n  <Networking>Enable</Networking>\n  <MemoryInMB>4096</MemoryInMB>\n  {mapped}\n</Configuration>"
    );
    std::fs::write(&wsb_path, content)?;
    Command::new("WindowsSandbox.exe")
        .arg(&wsb_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

// ── Hyper-V ─────────────────────────────────────────────────────

#[cfg(windows)]
pub fn launch_hyperv(vm_name: &str) -> Result<()> {
    let cmd = format!("Start-VM -Name '{vm_name}' -ErrorAction Stop");
    let status = Command::new("powershell").args(["-NoProfile", "-Command", &cmd]).status()?;
    if !status.success() {
        bail!("Start-VM failed for '{vm_name}'");
    }
    Ok(())
}

// ── VirtualBox (cross-platform) ─────────────────────────────────

pub fn launch_virtualbox(vm_name: &str) -> Result<()> {
    Command::new("VBoxManage")
        .args(["startvm", vm_name, "--type", "gui"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

// ── QEMU/KVM (Linux native) ─────────────────────────────────────

#[cfg(unix)]
pub fn launch_qemu(vm_name: Option<&str>, disk_path: Option<&Path>) -> Result<()> {
    let name = vm_name.unwrap_or("TsukuyomiOS");
    let data_dir = crate::store::ensure_data_dir()?;
    let vm_dir = data_dir.join("vm");
    let default_disk = vm_dir.join(format!("{name}.qcow2"));
    let disk = disk_path
        .map(|p| p.to_path_buf())
        .unwrap_or(default_disk);

    if !disk.exists() {
        bail!("Disk image not found: {}. Build a VM first.", disk.display());
    }

    let disk_str = disk.to_string_lossy().to_string();
    let ram = "4096";
    let cpus = "2";

    // Check for KVM acceleration
    let has_kvm = std::path::Path::new("/dev/kvm").exists();
    let accel_args: Vec<&str> = if has_kvm {
        vec!["-enable-kvm"]
    } else {
        vec!["-accel", "tc"]
    };

    let mut args: Vec<String> = vec![
        "-name".to_string(), name.to_string(),
        "-machine".to_string(), "type=q35".to_string(),
        "-m".to_string(), ram.to_string(),
        "-smp".to_string(), format!("cpus={cpus}"),
        "-drive".to_string(), format!("file={disk_str},format=qcow2,if=virtio"),
        "-display".to_string(), "gtk".to_string(),
        "-serial".to_string(), "pty".to_string(),
    ];
    args.extend(accel_args.iter().map(|s| s.to_string()));

    Command::new("qemu-system-x86_64")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(unix)]
pub fn create_qemu_vm(vm_name: &str, disk_path: &Path) -> Result<()> {
    if !disk_path.exists() {
        // Create a qcow2 disk image
        let disk_str = disk_path.to_string_lossy().to_string();
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2", &disk_str, "8G"])
            .status()?;
        if !status.success() {
            bail!("qemu-img create failed for {disk_str}");
        }
    }
    Ok(())
}

// ── VMware (cross-platform) ─────────────────────────────────────

pub fn launch_vmware(vm_name: Option<&str>) -> Result<()> {
    let name = vm_name.unwrap_or("TsukuyomiOS");
    if let Some(vmrun) = which_path("vmrun") {
        let status = Command::new(vmrun)
            .args(["start", &format!("{name}.vmx"), "gui"])
            .status()?;
        if !status.success() {
            bail!("vmrun start failed for '{name}'");
        }
        Ok(())
    } else {
        bail!("vmrun not found on PATH");
    }
}

fn which_path(name: &str) -> Option<std::path::PathBuf> {
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

// ── VirtualBox VM creation (cross-platform) ─────────────────────

fn run_checked(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program).args(args).status()?;
    if !status.success() {
        bail!("{program} {args:?} failed");
    }
    Ok(())
}

pub fn create_virtualbox_vm(vm_name: &str, disk_path: &Path) -> Result<()> {
    if !disk_path.exists() {
        bail!("Disk image not found: {}", disk_path.display());
    }

    let already_registered = Command::new("VBoxManage")
        .args(["showvminfo", vm_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if already_registered {
        return Ok(());
    }

    run_checked("VBoxManage", &["createvm", "--name", vm_name, "--ostype", "Linux_64", "--register"])?;
    run_checked(
        "VBoxManage",
        &[
            "modifyvm", vm_name, "--memory", "4096", "--cpus", "2", "--nic1", "nat", "--boot1",
            "disk", "--boot2", "none",
        ],
    )?;
    run_checked(
        "VBoxManage",
        &["storagectl", vm_name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"],
    )?;
    let disk_str = disk_path.to_string_lossy().to_string();
    run_checked(
        "VBoxManage",
        &[
            "storageattach", vm_name, "--storagectl", "SATA", "--port", "0", "--device", "0",
            "--type", "hdd", "--medium", &disk_str,
        ],
    )?;
    Ok(())
}

// ── Dispatch ────────────────────────────────────────────────────

pub fn launch_vm(backend_id: &str, vm_name: Option<&str>, mapped_folder: Option<&Path>) -> Result<()> {
    match backend_id {
        #[cfg(windows)]
        "windows_sandbox" => launch_windows_sandbox(mapped_folder),
        #[cfg(windows)]
        "hyperv" => launch_hyperv(vm_name.unwrap_or("TsukuyomiOS")),
        "virtualbox" => launch_virtualbox(vm_name.unwrap_or("TsukuyomiOS")),
        "vmware" => launch_vmware(vm_name),
        #[cfg(unix)]
        "qemu" | "kvm" => launch_qemu(vm_name, None),
        other => bail!("Launch not implemented for backend: {other}"),
    }
}