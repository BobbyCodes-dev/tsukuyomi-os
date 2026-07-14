from __future__ import annotations

import platform
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Optional


def detect_vm_backends() -> dict:
    """Return availability of each VM backend on this machine."""
    backends = {
        "windows_sandbox": False,
        "hyperv": False,
        "virtualbox": False,
        "vmware": False,
        "qemu": False,
    }

    if platform.system() == "Windows":
        # Windows Sandbox requires Pro/Enterprise and optional feature
        if shutil.which("WindowsSandbox.exe"):
            backends["windows_sandbox"] = True
        # Hyper-V
        try:
            result = subprocess.run(
                ["powershell", "-Command", "(Get-WindowsOptionalFeature -FeatureName Microsoft-Hyper-V-All -Online).State"],
                capture_output=True,
                text=True,
                timeout=10,
            )
            if "Enabled" in result.stdout:
                backends["hyperv"] = True
        except Exception:
            pass

    # Cross-platform
    if shutil.which("VBoxManage") or shutil.which("VBoxManage.exe"):
        backends["virtualbox"] = True
    if shutil.which("vmrun") or shutil.which("vmrun.exe"):
        backends["vmware"] = True
    if shutil.which("qemu-system-x86_64") or shutil.which("qemu-system-x86_64.exe"):
        backends["qemu"] = True

    return backends


def launch_windows_sandbox(mapped_folder: Optional[Path] = None) -> subprocess.Popen:
    """Launch Windows Sandbox with an optional mapped folder."""
    wsb = tempfile.NamedTemporaryFile(suffix=".wsb", delete=False, mode="w")
    mapped = f"<MappedFolder>\n      <HostFolder>{mapped_folder}</HostFolder>\n      <ReadOnly>false</ReadOnly>\n    </MappedFolder>" if mapped_folder else ""
    wsb.write(
        f"""<Configuration>
  <vGPU>Enable</vGPU>
  <Networking>Enable</Networking>
  <MemoryInMB>4096</MemoryInMB>
  {mapped}
</Configuration>"""
    )
    wsb.close()
    return subprocess.Popen(["WindowsSandbox.exe", wsb.name])


def launch_virtualbox(vm_name: str) -> subprocess.Popen:
    """Start a VirtualBox VM by name."""
    return subprocess.Popen(["VBoxManage", "startvm", vm_name, "--type", "gui"])


def launch_hyperv(vm_name: str) -> None:
    """Start a Hyper-V VM by name."""
    subprocess.run(["powershell", "-Command", f"Start-VM -Name '{vm_name}'"], check=True)


def launch_vm(backend: str, **kwargs) -> Optional[subprocess.Popen]:
    """Launch a VM using the requested backend."""
    if backend == "windows_sandbox":
        return launch_windows_sandbox(kwargs.get("mapped_folder"))
    if backend == "virtualbox":
        return launch_virtualbox(kwargs.get("vm_name", "TsukuyomiSandbox"))
    if backend == "hyperv":
        launch_hyperv(kwargs.get("vm_name", "TsukuyomiSandbox"))
        return None
    raise ValueError(f"Unknown VM backend: {backend}")
