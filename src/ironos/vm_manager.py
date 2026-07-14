from __future__ import annotations

import platform
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from ironos.local_store import data_dir


@dataclass
class VMBackend:
    id: str
    name: str
    available: bool
    reason: str
    vm_path: Optional[Path] = None


def _windows_edition() -> str:
    if platform.system() != "Windows":
        return ""
    try:
        result = subprocess.run(
            ["powershell", "-Command", "(Get-WindowsEdition -Online).Edition"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        return result.stdout.strip()
    except Exception:
        return ""


def _has_feature(feature: str) -> bool:
    if platform.system() != "Windows":
        return False
    try:
        result = subprocess.run(
            ["powershell", "-Command", f"(Get-WindowsOptionalFeature -FeatureName {feature} -Online).State"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        return "Enabled" in result.stdout
    except Exception:
        return False


def _which(name: str) -> Optional[str]:
    if platform.system() == "Windows":
        for ext in ("", ".exe", ".cmd", ".bat"):
            path = shutil.which(name + ext)
            if path:
                return path
        return None
    return shutil.which(name)


def detect_backends() -> list[VMBackend]:
    edition = _windows_edition()

    sandbox_available = False
    sandbox_reason = "Requires Windows 10/11 Pro/Enterprise"
    if platform.system() == "Windows":
        if edition in ("Professional", "Enterprise", "Education"):
            if _which("WindowsSandbox.exe"):
                if _has_feature("Containers-DisposableClientVM"):
                    sandbox_available = True
                    sandbox_reason = "Available"
                else:
                    sandbox_reason = "Windows Sandbox feature not enabled"
            else:
                sandbox_reason = "Windows Sandbox executable not found"
        else:
            sandbox_reason = f"Windows {edition} does not include Windows Sandbox"

    hyperv_available = False
    hyperv_reason = "Requires Windows Pro/Enterprise"
    if platform.system() == "Windows":
        if edition in ("Professional", "Enterprise", "Education"):
            if _has_feature("Microsoft-Hyper-V-All"):
                hyperv_available = True
                hyperv_reason = "Available"
            else:
                hyperv_reason = "Hyper-V feature not enabled"
        else:
            hyperv_reason = f"Windows {edition} does not include Hyper-V"

    vbox_manage = _which("VBoxManage")
    vbox_available = bool(vbox_manage)
    vbox_reason = "VirtualBox found" if vbox_available else "VirtualBox not installed (download from virtualbox.org)"

    vmware_path = _which("vmrun") or _which("vmplayer")
    vmware_available = bool(vmware_path)
    vmware_reason = "VMware found" if vmware_available else "VMware not installed"

    qemu_path = _which("qemu-system-x86_64")
    qemu_available = bool(qemu_path)
    qemu_reason = "QEMU found" if qemu_available else "QEMU not installed"

    return [
        VMBackend("windows_sandbox", "Windows Sandbox", sandbox_available, sandbox_reason),
        VMBackend("hyperv", "Hyper-V", hyperv_available, hyperv_reason),
        VMBackend("virtualbox", "VirtualBox", vbox_available, vbox_reason),
        VMBackend("vmware", "VMware", vmware_available, vmware_reason),
        VMBackend("qemu", "QEMU/KVM", qemu_available, qemu_reason),
    ]


def choose_backend(backends: list[VMBackend], prefer: Optional[str] = None) -> Optional[VMBackend]:
    order = [prefer] if prefer else []
    order += ["windows_sandbox", "hyperv", "virtualbox", "vmware", "qemu"]
    by_id = {b.id: b for b in backends}
    for bid in order:
        b = by_id.get(bid)
        if b and b.available:
            return b
    return None


def suggest_action(backends: list[VMBackend]) -> str:
    edition = _windows_edition()
    chosen = choose_backend(backends)
    if chosen:
        return f"Best available backend: {chosen.name}. Press Enter to launch."
    if platform.system() == "Windows" and edition == "Home":
        return "Windows Home detected. Install VirtualBox (virtualbox.org) to run Tsukuyomi OS in a sandboxed VM."
    return "No VM backend found. Install VirtualBox, VMware, or QEMU, or enable Windows Sandbox/Hyper-V."


def launch_windows_sandbox(mapped_folder: Optional[Path] = None) -> subprocess.Popen:
    wsb_path = data_dir() / "tsukuyomi.wsb"
    mapped = ""
    if mapped_folder and mapped_folder.exists():
        mapped = f"""
    <MappedFolder>
      <HostFolder>{mapped_folder}</HostFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>"""
    wsb_path.write_text(
        f"""<Configuration>
  <vGPU>Enable</vGPU>
  <Networking>Enable</Networking>
  <MemoryInMB>4096</MemoryInMB>
  {mapped}
</Configuration>""",
        encoding="utf-8",
    )
    return subprocess.Popen(["WindowsSandbox.exe", str(wsb_path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def launch_hyperv(vm_name: str = "TsukuyomiOS") -> None:
    subprocess.run(
        ["powershell", "-Command", f"Start-VM -Name '{vm_name}' -ErrorAction Stop"],
        check=True,
    )


def launch_virtualbox(vm_name: str = "TsukuyomiOS") -> subprocess.Popen:
    return subprocess.Popen(
        ["VBoxManage", "startvm", vm_name, "--type", "gui"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def create_virtualbox_vm(vm_name: str = "TsukuyomiOS", disk_path: Optional[Path] = None) -> None:
    if not disk_path or not disk_path.exists():
        raise FileNotFoundError(f"Disk image not found: {disk_path}")
    try:
        subprocess.run(["VBoxManage", "showvminfo", vm_name], capture_output=True, check=True)
        return
    except subprocess.CalledProcessError:
        pass
    subprocess.run(
        ["VBoxManage", "createvm", "--name", vm_name, "--ostype", "Linux_64", "--register"],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "modifyvm", vm_name, "--memory", "4096", "--cpus", "2", "--nic1", "nat", "--boot1", "disk", "--boot2", "none"],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "storagectl", vm_name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "storageattach", vm_name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd", "--medium", str(disk_path)],
        check=True,
    )


def launch_vm(backend_id: str, **kwargs) -> Optional[subprocess.Popen]:
    if backend_id == "windows_sandbox":
        return launch_windows_sandbox(kwargs.get("mapped_folder"))
    if backend_id == "hyperv":
        launch_hyperv(kwargs.get("vm_name", "TsukuyomiOS"))
        return None
    if backend_id == "virtualbox":
        return launch_virtualbox(kwargs.get("vm_name", "TsukuyomiOS"))
    raise ValueError(f"Launch not implemented for backend: {backend_id}")
