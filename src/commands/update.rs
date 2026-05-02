use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::*;

pub(crate) fn update(
    manifest_path: PathBuf,
    resource: Option<String>,
    check: bool,
    dry_run: bool,
) -> Result<()> {
    if check && dry_run {
        bail!("--dry-run cannot be combined with --check");
    }
    let manifest = load_valid_manifest(&manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let lockfile = if let Some(resource_id) = resource.as_deref() {
        build_selective_lockfile(&manifest, manifest_dir, resource_id)?
    } else {
        build_lockfile(&manifest, manifest_dir)?
    };
    let lockfile_path = manifest_dir.join("agentics.lock.yaml");
    let yaml = serde_yml::to_string(&lockfile).context("failed to serialize lockfile")?;
    if check {
        let existing = fs::read_to_string(&lockfile_path)
            .with_context(|| format!("failed to read lockfile `{}`", lockfile_path.display()))?;
        if existing != yaml {
            bail!("lockfile is out of date: {}", lockfile_path.display());
        }
        println!("Lockfile OK: {}", lockfile_path.display());
        return Ok(());
    }
    if dry_run {
        println!("Dry-run lockfile for {}:\n{yaml}", lockfile_path.display());
        return Ok(());
    }
    write_file_atomically(&lockfile_path, yaml.as_bytes())?;
    println!("Updated {}", lockfile_path.display());
    Ok(())
}
