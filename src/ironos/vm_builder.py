from __future__ import annotations

import argparse
import hashlib
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Optional

from ironos.local_store import data_dir


def download_file(url: str, dest: Path) -> None:
    """Download url to dest using curl or wget."""
    for tool in ("curl", "wget"):
        exe = shutil.which(tool)
        if exe:
            if tool == "curl":
                subprocess.run([exe, "-fsSL", "-o", str(dest), url], check=True)
            else:
                subprocess.run([exe, "-O", str(dest), url], check=True)
            return
    raise RuntimeError("curl or wget required to download VM image")


def build_alpine_vm(dest_vdi: Path, alpine_version: str = "v3.19", arch: str = "x86_64") -> None:
    """Create a minimal Alpine Linux VirtualBox VM disk with Tsukuyomi OS preinstalled."""
    if not shutil.which("VBoxManage"):
        raise RuntimeError("VirtualBox is required to build the VM image")

    tmp = Path(tempfile.mkdtemp(prefix="tsukuyomi-build-"))
    iso = tmp / f"alpine-standard-{alpine_version}-{arch}.iso"
    base_url = f"https://dl-cdn.alpinelinux.org/alpine/{alpine_version}/releases/{arch}"
    iso_url = f"{base_url}/alpine-standard-3.19.1-{arch}.iso"

    print(f"Downloading Alpine ISO to {iso} ...")
    download_file(iso_url, iso)

    vdi = tmp / "tsukuyomi-base.vdi"
    subprocess.run(
        ["VBoxManage", "createmedium", "disk", "--filename", str(vdi), "--size", "20480", "--variant", "Standard"],
        check=True,
    )

    vm_name = "TsukuyomiBuilder"
    try:
        subprocess.run(["VBoxManage", "unregistervm", vm_name, "--delete"], capture_output=True)
    except Exception:
        pass

    subprocess.run(
        ["VBoxManage", "createvm", "--name", vm_name, "--ostype", "Linux_64", "--register"],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "modifyvm", vm_name, "--memory", "2048", "--cpus", "2", "--nic1", "nat"],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "storagectl", vm_name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "storageattach", vm_name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd", "--medium", str(vdi)],
        check=True,
    )
    subprocess.run(
        ["VBoxManage", "storageattach", vm_name, "--storagectl", "SATA", "--port", "1", "--device", "0", "--type", "dvddrive", "--medium", str(iso)],
        check=True,
    )

    print("Starting VM to install Alpine...")
    subprocess.run(["VBoxManage", "startvm", vm_name, "--type", "headless"], check=True)
    print("Installation must be automated via answer file or manual. This skeleton stops here.")
    print(f"Base VDI: {vdi}")


def build_or_download_vm(dest_dir: Path) -> Path:
    """Return a ready-to-use Tsukuyomi OS VM disk. Build it if missing."""
    dest_dir.mkdir(parents=True, exist_ok=True)
    vdi = dest_dir / "tsukuyomi-os-base.vdi"
    if vdi.exists():
        return vdi
    print("No prebuilt VM disk found. Build one with: tsukuyomi build-vm")
    return vdi


def main() -> None:
    parser = argparse.ArgumentParser("Tsukuyomi OS VM builder")
    parser.add_argument("--dest", default=str(data_dir() / "vm"), help="Output directory")
    args = parser.parse_args()
    dest = Path(args.dest)
    build_alpine_vm(dest / "tsukuyomi-os-base.vdi")


if __name__ == "__main__":
    main()
