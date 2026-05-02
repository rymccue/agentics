use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    commands::sync::{SyncOptions, sync},
    *,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct RefreshOptions {
    pub(crate) harness: Option<HarnessName>,
    pub(crate) force: bool,
    pub(crate) yes: bool,
    pub(crate) adopt_existing: bool,
    pub(crate) non_interactive: bool,
}

pub(crate) fn refresh(manifest_path: PathBuf, options: RefreshOptions) -> Result<()> {
    let manifest = load_valid_manifest(&manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    write_lockfile(&manifest, manifest_dir)?;
    sync(
        manifest_path,
        SyncOptions {
            dry_run: false,
            json: false,
            harness: options.harness,
            global: false,
            force: options.force,
            yes: options.yes,
            write_lock: false,
            adopt_existing: options.adopt_existing,
            non_interactive: options.non_interactive,
        },
    )
}
