use anyhow::{bail, Result};
use std::path::Path;
use std::process::{Command, Stdio};

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

pub fn launch_hyperv(vm_name: &str) -> Result<()> {
    let cmd = format!("Start-VM -Name '{vm_name}' -ErrorAction Stop");
    let status = Command::new("powershell").args(["-NoProfile", "-Command", &cmd]).status()?;
    if !status.success() {
        bail!("Start-VM failed for '{vm_name}'");
    }
    Ok(())
}

pub fn launch_virtualbox(vm_name: &str) -> Result<()> {
    Command::new("VBoxManage")
        .args(["startvm", vm_name, "--type", "gui"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

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

pub fn launch_vm(backend_id: &str, vm_name: Option<&str>, mapped_folder: Option<&Path>) -> Result<()> {
    match backend_id {
        "windows_sandbox" => launch_windows_sandbox(mapped_folder),
        "hyperv" => launch_hyperv(vm_name.unwrap_or("TsukuyomiOS")),
        "virtualbox" => launch_virtualbox(vm_name.unwrap_or("TsukuyomiOS")),
        other => bail!("Launch not implemented for backend: {other}"),
    }
}
