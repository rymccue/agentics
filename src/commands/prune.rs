use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::commands::list::declared_resource_targets;
use crate::*;

pub(crate) fn prune(manifest_path: PathBuf, dry_run: bool) -> Result<()> {
    let manifest = load_valid_manifest(&manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let metadata_path = manifest_dir.join(".agentics/installed.yaml");
    if !metadata_path.is_file() {
        println!("No installed metadata found.");
        return Ok(());
    }
    let text = fs::read_to_string(&metadata_path)
        .with_context(|| format!("failed to read metadata `{}`", metadata_path.display()))?;
    let mut summary: InstalledSummary = serde_yml::from_str(&text)
        .with_context(|| format!("failed to parse metadata `{}`", metadata_path.display()))?;
    let declared_targets = declared_resource_targets(&manifest)
        .into_iter()
        .map(|entry| entry.target)
        .collect::<BTreeSet<_>>();
    let mut kept = Vec::new();
    let mut pruned = 0usize;
    for entry in summary.installed {
        if declared_targets.contains(&entry.target) {
            kept.push(entry);
            continue;
        }
        pruned += 1;
        if dry_run {
            println!("would prune {}", entry.target);
            kept.push(entry);
        } else {
            prune_installed_entry(manifest_dir, &entry)?;
            println!("pruned {}", entry.target);
        }
    }
    if pruned == 0 {
        println!("No stale managed resources.");
        return Ok(());
    }
    if !dry_run {
        summary.installed = kept;
        let yaml =
            serde_yml::to_string(&summary).context("failed to serialize installed metadata")?;
        write_file_atomically(&metadata_path, yaml.as_bytes())?;
    }
    Ok(())
}

fn prune_installed_entry(manifest_dir: &Path, entry: &InstalledSummaryEntry) -> Result<()> {
    let target = manifest_dir.join(&entry.target);
    ensure_safe_destination(manifest_dir, &target)?;
    if !target.exists() {
        return Ok(());
    }
    match entry.kind.as_str() {
        "directory" => {
            let metadata_path = target.join(".agentics-owner");
            if !metadata_path.is_file() {
                bail!("refusing to prune unmanaged target `{}`", entry.target);
            }
            remove_path(&target)
        }
        "file" => {
            let metadata_path = target.with_extension("agentics-owner");
            if !metadata_path.is_file() {
                bail!("refusing to prune unmanaged target `{}`", entry.target);
            }
            remove_path(&target)?;
            if metadata_path.exists() {
                remove_path(&metadata_path)?;
            }
            Ok(())
        }
        other => bail!("unsupported installed metadata kind `{other}`"),
    }
}
