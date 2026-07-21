use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use super::network::NetworkMode;

const RELEASES_INDEX_URL: &str =
    "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/latest-releases.yaml";
const RELEASES_BASE_URL: &str = "https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64";
const TARGET_FLAVOR: &str = "alpine-virt";

#[derive(Debug)]
pub enum BuildEvent {
    Status(String),
    Done(PathBuf),
    Error(String),
}

struct ReleaseEntry {
    flavor: String,
    file: String,
    sha256: String,
}

fn parse_latest_releases(text: &str) -> Vec<ReleaseEntry> {
    let mut entries = Vec::new();
    let mut flavor = String::new();
    let mut file = String::new();
    let mut sha256 = String::new();
    let mut have_entry = false;

    let flush = |have_entry: bool, flavor: &str, file: &str, sha256: &str, out: &mut Vec<ReleaseEntry>| {
        if have_entry && !flavor.is_empty() {
            out.push(ReleaseEntry {
                flavor: flavor.to_string(),
                file: file.to_string(),
                sha256: sha256.to_string(),
            });
        }
    };

    for line in text.lines() {
        if line.trim() == "-" {
            flush(have_entry, &flavor, &file, &sha256, &mut entries);
            flavor.clear();
            file.clear();
            sha256.clear();
            have_entry = true;
            continue;
        }
        let is_top_level = line.starts_with("  ") && !line.starts_with("   ");
        if !is_top_level {
            continue;
        }
        let rest = &line[2..];
        if let Some(v) = rest.strip_prefix("flavor:") {
            flavor = v.trim().trim_matches('"').to_string();
        } else if let Some(v) = rest.strip_prefix("file:") {
            file = v.trim().trim_matches('"').to_string();
        } else if let Some(v) = rest.strip_prefix("sha256:") {
            sha256 = v.trim().trim_matches('"').to_string();
        }
    }
    flush(have_entry, &flavor, &file, &sha256, &mut entries);
    entries
}

// ── Cross-platform HTTP fetch ───────────────────────────────────

fn curl_fetch_text(url: &str) -> Result<String> {
    let output = Command::new("curl")
        .args(["-fsSL", url])
        .output()?;
    if !output.status.success() {
        bail!(
            "Failed to fetch {url}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(windows)]
fn powershell_fetch_text(url: &str) -> Result<String> {
    let ps = format!(
        "$ProgressPreference='SilentlyContinue'; (Invoke-WebRequest -UseBasicParsing -Uri '{url}').Content"
    );
    let output = Command::new("powershell").args(["-NoProfile", "-Command", &ps]).output()?;
    if !output.status.success() {
        bail!("Failed to fetch {url}: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn fetch_text(url: &str) -> Result<String> {
    // Try curl first (available on both Linux and modern Windows)
    if Command::new("curl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        curl_fetch_text(url)
    } else {
        #[cfg(windows)]
        {
            powershell_fetch_text(url)
        }
        #[cfg(unix)]
        {
            bail!("curl is required but not found on PATH")
        }
    }
}

fn curl_download_file(url: &str, dest: &Path) -> Result<()> {
    let dest_str = dest.to_string_lossy().to_string();
    let status = Command::new("curl")
        .args(["-fsSL", "-o", &dest_str, url])
        .status()?;
    if !status.success() {
        bail!("Failed to download {url}");
    }
    Ok(())
}

#[cfg(windows)]
fn powershell_download_file(url: &str, dest: &Path) -> Result<()> {
    let dest_str = dest.to_string_lossy().to_string();
    let ps = format!(
        "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile '{dest_str}'"
    );
    let status = Command::new("powershell").args(["-NoProfile", "-Command", &ps]).status()?;
    if !status.success() {
        bail!("Failed to download {url}");
    }
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<()> {
    if Command::new("curl")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        curl_download_file(url, dest)
    } else {
        #[cfg(windows)]
        {
            powershell_download_file(url, dest)
        }
        #[cfg(unix)]
        {
            bail!("curl is required but not found on PATH")
        }
    }
}

// ── Checksum ────────────────────────────────────────────────────

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

// ── VirtualBox helpers (cross-platform) ─────────────────────────

fn vbox(args: &[&str]) -> Result<()> {
    let status = Command::new("VBoxManage").args(args).status()?;
    if !status.success() {
        bail!("VBoxManage {args:?} failed");
    }
    Ok(())
}

fn vbox_owned(args: &[String]) -> Result<()> {
    let status = Command::new("VBoxManage").args(args).status()?;
    if !status.success() {
        bail!("VBoxManage {args:?} failed");
    }
    Ok(())
}

fn vm_registered(vm_name: &str) -> bool {
    Command::new("VBoxManage")
        .args(["showvminfo", vm_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn create_vm_shell(
    vm_name: &str,
    vdi_path: &Path,
    iso_path: &Path,
    network: NetworkMode,
    log_path: &Path,
) -> Result<()> {
    let already_registered = vm_registered(vm_name);

    if !already_registered {
        vbox(&["createvm", "--name", vm_name, "--ostype", "Linux_64", "--register"])?;
    }

    if !vdi_path.exists() {
        let vdi_str = vdi_path.to_string_lossy().to_string();
        vbox(&["createmedium", "disk", "--filename", &vdi_str, "--size", "8192", "--format", "VDI"])?;
    }

    let mut modify: Vec<String> = vec![
        "modifyvm".to_string(),
        vm_name.to_string(),
        "--memory".to_string(),
        "2048".to_string(),
        "--cpus".to_string(),
        "2".to_string(),
        "--boot1".to_string(),
        "dvd".to_string(),
        "--boot2".to_string(),
        "disk".to_string(),
        "--boot3".to_string(),
        "none".to_string(),
        "--boot4".to_string(),
        "none".to_string(),
        "--audio".to_string(),
        "none".to_string(),
        "--uart1".to_string(),
        "0x3F8".to_string(),
        "4".to_string(),
        "--uartmode1".to_string(),
        "file".to_string(),
        log_path.to_string_lossy().to_string(),
    ];
    modify.extend(network.nic_args());
    vbox_owned(&modify)?;

    let _ = vbox(&[
        "storagectl", vm_name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci", "--portcount", "2",
    ]);

    let vdi_str = vdi_path.to_string_lossy().to_string();
    vbox(&[
        "storageattach", vm_name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd",
        "--medium", &vdi_str,
    ])?;

    let iso_str = iso_path.to_string_lossy().to_string();
    vbox(&[
        "storageattach", vm_name, "--storagectl", "SATA", "--port", "1", "--device", "0", "--type", "dvddrive",
        "--medium", &iso_str,
    ])?;

    Ok(())
}

fn poll_for_marker(log_path: &Path, marker: &str, timeout: Duration, interval: Duration) -> bool {
    let start = Instant::now();
    loop {
        if let Ok(content) = std::fs::read_to_string(log_path) {
            if content.contains(marker) {
                return true;
            }
        }
        if start.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(interval);
    }
}

fn wait_for_boot_shell(vm_name: &str, log_path: &Path, tx: &Sender<BuildEvent>) -> Result<()> {
    let _ = tx.send(BuildEvent::Status("Waiting for the Alpine live console to become ready...".to_string()));
    let marker = "TSUKUYOMI_BOOT_READY";
    let start = Instant::now();
    let timeout = Duration::from_secs(240);
    let probe_interval = Duration::from_secs(5);
    loop {
        let _ = super::scancode::type_string(vm_name, &format!("echo {marker} > /dev/ttyS0\n"));
        std::thread::sleep(probe_interval);
        if let Ok(content) = std::fs::read_to_string(log_path) {
            if content.contains(marker) {
                return Ok(());
            }
        }
        if start.elapsed() > timeout {
            bail!(
                "Timed out waiting for the Alpine installer console to respond. Check that the VM \
                 booted from the ISO (VirtualBox boot order) and that VBoxManage keyboard injection \
                 is reaching a root shell."
            );
        }
    }
}

fn build_answerfile() -> String {
    [
        "KEYMAPOPTS=none",
        "HOSTNAMEOPTS=tsukuyomi-sandbox",
        "INTERFACESOPTS=none",
        "DNSOPTS=\"-d local 127.0.0.1\"",
        "TIMEZONEOPTS=UTC",
        "PROXYOPTS=none",
        "APKREPOSOPTS=\"/media/cdrom/apks\"",
        "USEROPTS=none",
        "SSHDOPTS=none",
        "NTPOPTS=none",
        "ERASE_DISKS=/dev/sda",
        "DISKOPTS=\"-m sys /dev/sda\"",
        "LBUOPTS=none",
        "APKCACHEOPTS=none",
        "",
    ]
    .join("\n")
}

fn run_unattended_install(vm_name: &str, log_path: &Path, tx: &Sender<BuildEvent>) -> Result<()> {
    wait_for_boot_shell(vm_name, log_path, tx)?;

    let _ = tx.send(BuildEvent::Status("Writing unattended answer file...".to_string()));
    let answerfile = build_answerfile();
    let write_cmd = format!(
        "cat > /tmp/tsukuyomi-answers <<'TSUKEOF'\n{answerfile}TSUKEOF\necho TSUKUYOMI_STAGE_ANSWERFILE_DONE > /dev/ttyS0\n"
    );
    super::scancode::type_string(vm_name, &write_cmd)?;
    if !poll_for_marker(log_path, "TSUKUYOMI_STAGE_ANSWERFILE_DONE", Duration::from_secs(30), Duration::from_millis(500)) {
        bail!("Timed out waiting for the unattended answer file to be written inside the VM.");
    }

    let _ = tx.send(BuildEvent::Status(
        "Running unattended Alpine install (offline, from ISO repo)... this can take several minutes.".to_string(),
    ));
    let install_cmd = "setup-alpine -ef /tmp/tsukuyomi-answers > /tmp/tsukuyomi-setup.log 2>&1 && echo TSUKUYOMI_STAGE_INSTALL_DONE > /dev/ttyS0 || echo TSUKUYOMI_STAGE_INSTALL_FAILED > /dev/ttyS0\n";
    super::scancode::type_string(vm_name, install_cmd)?;

    let start = Instant::now();
    let timeout = Duration::from_secs(20 * 60);
    loop {
        if let Ok(content) = std::fs::read_to_string(log_path) {
            if content.contains("TSUKUYOMI_STAGE_INSTALL_DONE") {
                break;
            }
            if content.contains("TSUKUYOMI_STAGE_INSTALL_FAILED") {
                bail!(
                    "Unattended Alpine install reported failure inside the VM. Check \
                     /tmp/tsukuyomi-setup.log on the guest (not retrievable from the host automatically)."
                );
            }
        }
        if start.elapsed() > timeout {
            bail!("Timed out waiting for the unattended Alpine install to finish.");
        }
        std::thread::sleep(Duration::from_secs(3));
    }

    let _ = tx.send(BuildEvent::Status("Install finished, shutting down the VM...".to_string()));
    super::scancode::type_string(vm_name, "poweroff\n")?;
    Ok(())
}

fn wait_for_vm_stopped(vm_name: &str, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    let needle = format!("\"{vm_name}\"");
    loop {
        let output = Command::new("VBoxManage").args(["list", "runningvms"]).output()?;
        let listing = String::from_utf8_lossy(&output.stdout);
        if !listing.contains(&needle) {
            return Ok(());
        }
        if start.elapsed() > timeout {
            bail!("Timed out waiting for VM '{vm_name}' to power off.");
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

// ── QEMU/KVM build path (Linux) ─────────────────────────────────

#[cfg(unix)]
fn build_qemu_vm(
    dest_dir: &Path,
    vm_name: &str,
    iso_path: &Path,
    tx: &Sender<BuildEvent>,
) -> Result<PathBuf> {
    let qcow2_path = dest_dir.join(format!("{vm_name}.qcow2"));

    if !qcow2_path.exists() {
        let _ = tx.send(BuildEvent::Status("Creating QEMU disk image (8GB qcow2)...".to_string()));
        let disk_str = qcow2_path.to_string_lossy().to_string();
        let status = Command::new("qemu-img")
            .args(["create", "-f", "qcow2", &disk_str, "8G"])
            .status()?;
        if !status.success() {
            bail!("qemu-img create failed for {disk_str}");
        }
    }

    let _ = tx.send(BuildEvent::Status("Booting Alpine installer via QEMU (headless)...".to_string()));
    let disk_str = qcow2_path.to_string_lossy().to_string();
    let iso_str = iso_path.to_string_lossy().to_string();

    let has_kvm = std::path::Path::new("/dev/kvm").exists();
    let mut qemu_args: Vec<String> = vec![
        "-name".to_string(), vm_name.to_string(),
        "-machine".to_string(), "type=q35".to_string(),
        "-m".to_string(), "2048".to_string(),
        "-smp".to_string(), "cpus=2".to_string(),
        "-drive".to_string(), format!("file={disk_str},format=qcow2,if=virtio"),
        "-cdrom".to_string(), iso_str,
        "-boot".to_string(), "d".to_string(),
        "-display".to_string(), "none".to_string(),
        "-serial".to_string(), "file".to_string(),
    ];

    let serial_log = dest_dir.join(format!("{vm_name}-install.log"));
    qemu_args.push(serial_log.to_string_lossy().to_string());

    if has_kvm {
        qemu_args.push("-enable-kvm".to_string());
    }

    let mut child = Command::new("qemu-system-x86_64")
        .args(&qemu_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    // Wait for boot, then send commands via serial (stdin)
    let _ = tx.send(BuildEvent::Status("Waiting for Alpine to boot in QEMU...".to_string()));
    std::thread::sleep(Duration::from_secs(30));

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        let _ = stdin.write_all(b"\n");
        std::thread::sleep(Duration::from_secs(3));
        let _ = stdin.write_all(b"setup-alpine -ef /tmp/tsukuyomi-answers\n");
    }

    let _ = tx.send(BuildEvent::Status(
        "QEMU VM running with Alpine installer. Interactive setup may be needed.".to_string(),
    ));

    // For QEMU, we return the qcow2 path — the VM will need manual interaction
    // or a more sophisticated serial automation approach
    Ok(qcow2_path)
}

// ── Main build entry point ──────────────────────────────────────

pub fn build_or_download_vm(
    dest_dir: &Path,
    vm_name: &str,
    network: NetworkMode,
    tx: Sender<BuildEvent>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;

    #[cfg(unix)]
    {
        // On Linux, prefer QEMU/KVM if available, fall back to VirtualBox
        let has_qemu = Command::new("which")
            .arg("qemu-system-x86_64")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        let has_vbox = Command::new("which")
            .arg("VBoxManage")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        // Use .vdi for VirtualBox, .qcow2 for QEMU
        let qcow2_path = dest_dir.join(format!("{vm_name}.qcow2"));
        let vdi_path = dest_dir.join(format!("{vm_name}.vdi"));

        if has_qemu && qcow2_path.exists() {
            let _ = tx.send(BuildEvent::Status(format!("Using existing QEMU disk image at {}", qcow2_path.display())));
            return Ok(qcow2_path);
        }

        if has_vbox && vdi_path.exists() {
            let _ = tx.send(BuildEvent::Status(format!("Using existing VirtualBox disk image at {}", vdi_path.display())));
            return Ok(vdi_path);
        }

        if has_qemu && !has_vbox {
            // QEMU-only path
            return build_qemu_vm_path(dest_dir, vm_name, network, tx);
        }
        // Fall through to VirtualBox path (or if both available, prefer VBox for unattended install)
    }

    // VirtualBox build path (cross-platform)
    let vdi_path = dest_dir.join(format!("{vm_name}.vdi"));

    if vdi_path.exists() {
        let _ = tx.send(BuildEvent::Status(format!("Using existing disk image at {}", vdi_path.display())));
        return Ok(vdi_path);
    }

    let _ = tx.send(BuildEvent::Status("Looking up latest Alpine 'virt' release...".to_string()));
    let index_text = fetch_text(RELEASES_INDEX_URL)?;
    let entries = parse_latest_releases(&index_text);
    let entry = entries
        .iter()
        .find(|e| e.flavor == TARGET_FLAVOR)
        .ok_or_else(|| anyhow!("Could not find the '{TARGET_FLAVOR}' flavor in the Alpine release index."))?;
    if entry.file.is_empty() || entry.sha256.is_empty() {
        bail!("Alpine release index entry for '{TARGET_FLAVOR}' is missing file/sha256 fields.");
    }

    let iso_path = dest_dir.join(&entry.file);
    let iso_url = format!("{RELEASES_BASE_URL}/{}", entry.file);

    let need_download = if iso_path.exists() {
        match sha256_file(&iso_path) {
            Ok(actual) => !actual.eq_ignore_ascii_case(&entry.sha256),
            Err(_) => true,
        }
    } else {
        true
    };

    if need_download {
        let _ = tx.send(BuildEvent::Status(format!("Downloading {} ...", entry.file)));
        download_file(&iso_url, &iso_path)?;
        let _ = tx.send(BuildEvent::Status("Verifying SHA256 checksum...".to_string()));
        let actual = sha256_file(&iso_path)?;
        if !actual.eq_ignore_ascii_case(&entry.sha256) {
            let _ = std::fs::remove_file(&iso_path);
            bail!(
                "SHA256 mismatch for {}: expected {}, got {actual}. Downloaded file was deleted.",
                entry.file,
                entry.sha256
            );
        }
    } else {
        let _ = tx.send(BuildEvent::Status("ISO already downloaded and checksum verified.".to_string()));
    }

    let _ = tx.send(BuildEvent::Status("Creating VirtualBox VM...".to_string()));
    let log_path = dest_dir.join(format!("{vm_name}-install.log"));
    let _ = std::fs::remove_file(&log_path);
    create_vm_shell(vm_name, &vdi_path, &iso_path, network, &log_path)?;

    let _ = tx.send(BuildEvent::Status("Booting Alpine installer (headless)...".to_string()));
    vbox(&["startvm", vm_name, "--type", "headless"])?;

    if let Err(e) = run_unattended_install(vm_name, &log_path, &tx) {
        let _ = tx.send(BuildEvent::Status(format!("Install automation failed: {e}. Powering off VM.")));
        let _ = vbox(&["controlvm", vm_name, "poweroff"]);
        let _ = wait_for_vm_stopped(vm_name, Duration::from_secs(60));
        return Err(e);
    }

    let _ = tx.send(BuildEvent::Status("Ejecting install media...".to_string()));
    let _ = vbox(&["storageattach", vm_name, "--storagectl", "SATA", "--port", "1", "--device", "0", "--medium", "none"]);

    let _ = tx.send(BuildEvent::Status("Waiting for the VM to power off...".to_string()));
    wait_for_vm_stopped(vm_name, Duration::from_secs(120))?;

    let _ = tx.send(BuildEvent::Status("VM build complete.".to_string()));
    Ok(vdi_path)
}

// ── QEMU build helper (Linux) ───────────────────────────────────

#[cfg(unix)]
fn build_qemu_vm_path(
    dest_dir: &Path,
    vm_name: &str,
    _network: NetworkMode,
    tx: Sender<BuildEvent>,
) -> Result<PathBuf> {
    let _ = tx.send(BuildEvent::Status("Looking up latest Alpine 'virt' release...".to_string()));
    let index_text = fetch_text(RELEASES_INDEX_URL)?;
    let entries = parse_latest_releases(&index_text);
    let entry = entries
        .iter()
        .find(|e| e.flavor == TARGET_FLAVOR)
        .ok_or_else(|| anyhow!("Could not find the '{TARGET_FLAVOR}' flavor in the Alpine release index."))?;

    let iso_path = dest_dir.join(&entry.file);
    let iso_url = format!("{RELEASES_BASE_URL}/{}", entry.file);

    let need_download = if iso_path.exists() {
        match sha256_file(&iso_path) {
            Ok(actual) => !actual.eq_ignore_ascii_case(&entry.sha256),
            Err(_) => true,
        }
    } else {
        true
    };

    if need_download {
        let _ = tx.send(BuildEvent::Status(format!("Downloading {} ...", entry.file)));
        download_file(&iso_url, &iso_path)?;
        let _ = tx.send(BuildEvent::Status("Verifying SHA256 checksum...".to_string()));
        let actual = sha256_file(&iso_path)?;
        if !actual.eq_ignore_ascii_case(&entry.sha256) {
            let _ = std::fs::remove_file(&iso_path);
            bail!(
                "SHA256 mismatch for {}: expected {}, got {actual}. Downloaded file was deleted.",
                entry.file,
                entry.sha256
            );
        }
    } else {
        let _ = tx.send(BuildEvent::Status("ISO already downloaded and checksum verified.".to_string()));
    }

    build_qemu_vm(dest_dir, vm_name, &iso_path, &tx)
}