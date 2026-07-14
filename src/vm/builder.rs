use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn build_or_download_vm(_dest_dir: &Path) -> Result<PathBuf> {
    bail!(
        "VM image building is not implemented yet in this version. \
         Provide a prebuilt VirtualBox disk manually, or build one using the legacy Python tooling."
    )
}
