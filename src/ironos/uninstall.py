#!/usr/bin/env python3
"""Tsukuyomi OS cleaner/uninstaller.

Removes all local data, installed package, and optionally VM disks.
Run with --keep-vms to preserve downloaded/build VM images.
"""

from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
import sys
from pathlib import Path

try:
    from ironos.local_store import data_dir
except Exception:
    from platformdirs import user_data_dir

    def data_dir() -> Path:
        return Path(user_data_dir("TsukuyomiOS", "bobbycodes"))


def nuke() -> None:
    parser = argparse.ArgumentParser("Tsukuyomi OS uninstaller")
    parser.add_argument("--keep-vms", action="store_true", help="Preserve VM disk images")
    parser.add_argument("--yes", action="store_true", help="Skip confirmation")
    args = parser.parse_args()

    data = data_dir()
    home = Path.home()

    targets = []
    if platform.system() == "Windows":
        targets = [
            data,
            home / "AppData" / "Local" / "TsukuyomiOS",
            home / "AppData" / "Roaming" / "TsukuyomiOS",
        ]
    else:
        targets = [
            data,
            home / ".local" / "share" / "TsukuyomiOS",
            home / ".config" / "TsukuyomiOS",
        ]

    existing = sorted({p.resolve() for p in targets if p.exists()})

    if not existing:
        print("Tsukuyomi OS data not found. Nothing to remove.")
        return

    print("This will PERMANENTLY delete the following Tsukuyomi OS data:")
    for p in existing:
        print(f"  {p}")
    if not args.yes:
        answer = input("Type 'NUKE' to confirm: ")
        if answer.strip() != "NUKE":
            print("Cancelled.")
            return

    for p in existing:
        try:
            if p.is_dir():
                if args.keep_vms and p.name == "vm":
                    print(f"Preserving VM directory: {p}")
                    continue
                shutil.rmtree(p, ignore_errors=True)
            else:
                p.unlink(missing_ok=True)
            print(f"Removed: {p}")
        except Exception as e:
            print(f"Failed to remove {p}: {e}")

    # Uninstall Python package if possible
    try:
        subprocess.run([sys.executable, "-m", "pip", "uninstall", "-y", "tsukuyomi-os"], check=False)
    except Exception as e:
        print(f"pip uninstall attempt failed: {e}")

    print("Tsukuyomi OS has been removed from this machine.")


if __name__ == "__main__":
    nuke()
