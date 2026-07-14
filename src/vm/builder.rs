use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// VM image building (Alpine ISO download + VirtualBox disk creation) was deferred
/// out of scope for this rewrite's first version. See the original `vm_builder.py`
/// for reference when this gets implemented.
pub fn build_or_download_vm(_dest_dir: &Path) -> Result<PathBuf> {
    bail!(
        "VM image building is not implemented yet in this version. \
         Provide a prebuilt VirtualBox disk manually, or build one using the legacy Python tooling."
    )
}
