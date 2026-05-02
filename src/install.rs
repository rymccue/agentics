use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledSummary {
    pub(crate) lockfile_hash: String,
    pub(crate) installed: Vec<InstalledSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledSummaryEntry {
    pub(crate) target: String,
    pub(crate) source: String,
    pub(crate) integrity: String,
    pub(crate) kind: String,
    pub(crate) owners: Vec<String>,
}

pub(crate) fn write_installed_summary(manifest_dir: &Path, actions: &[PlanAction]) -> Result<()> {
    let entries = actions
        .iter()
        .map(|action| InstalledSummaryEntry {
            target: action.target.display().to_string(),
            source: action.source.display().to_string(),
            integrity: action.integrity.clone(),
            kind: match action.kind {
                ActionKind::Directory => "directory".to_string(),
                ActionKind::File => "file".to_string(),
            },
            owners: action
                .owners
                .iter()
                .map(|owner| owner.as_str().to_string())
                .collect(),
        })
        .collect();
    let summary = InstalledSummary {
        lockfile_hash: lockfile_hash(manifest_dir)?,
        installed: entries,
    };
    let yaml = serde_yml::to_string(&summary).context("failed to serialize installed metadata")?;
    write_file_atomically(
        &manifest_dir.join(".agentics/installed.yaml"),
        yaml.as_bytes(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallOutcome {
    Changed,
    Current,
}

pub(crate) fn install_action(
    root: &Path,
    action: &PlanAction,
    force: bool,
) -> Result<InstallOutcome> {
    let source = if action.source.is_absolute() {
        action.source.clone()
    } else {
        root.join(&action.source)
    };
    let target = root.join(&action.target);
    check_write_preconditions(root, action, force)?;
    if target.exists() && target_state(&target, action)? == "installed" {
        return Ok(InstallOutcome::Current);
    }

    let temp_target = target.with_extension("agentics-tmp");
    if temp_target.exists() {
        remove_path(&temp_target)
            .with_context(|| format!("failed to remove temp target `{}`", temp_target.display()))?;
    }
    match action.kind {
        ActionKind::Directory => {
            copy_dir(&source, &temp_target)?;
            write_owner_metadata(&metadata_path_for(&temp_target, action.kind), action)?;
        }
        ActionKind::File => {
            if let Some(parent) = temp_target.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create directory `{}`", parent.display())
                })?;
            }
            fs::copy(&source, &temp_target).with_context(|| {
                format!(
                    "failed to copy `{}` to `{}`",
                    source.display(),
                    temp_target.display()
                )
            })?;
            write_owner_metadata(&metadata_path_for(&temp_target, action.kind), action)?;
        }
    }
    if target.exists() {
        remove_path(&target)
            .with_context(|| format!("failed to replace target `{}`", target.display()))?;
    }
    fs::rename(&temp_target, &target).with_context(|| {
        format!(
            "failed to move `{}` to `{}`",
            temp_target.display(),
            target.display()
        )
    })?;
    Ok(InstallOutcome::Changed)
}

pub(crate) fn check_write_preconditions(
    root: &Path,
    action: &PlanAction,
    force: bool,
) -> Result<()> {
    let target = root.join(&action.target);
    ensure_safe_destination(root, &target)?;
    let metadata_path = metadata_path_for(&target, action.kind);

    if target.exists() && !metadata_path.is_file() {
        bail!(
            "refusing to overwrite unmanaged target `{}`",
            action.target.display()
        );
    }
    if target.exists() {
        let current_integrity = hash_path(&target)?;
        let expected_integrity = read_metadata_value(&metadata_path, "integrity")?;
        if expected_integrity.as_deref() != Some(current_integrity.as_str()) && !force {
            bail!(
                "refusing to overwrite drifted managed target `{}`",
                action.target.display()
            );
        }
    }
    Ok(())
}

pub(crate) fn ensure_safe_destination(root: &Path, target: &Path) -> Result<()> {
    let root_input = if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    };
    let root = root_input
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root `{}`", root_input.display()))?;
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };
    let relative = target.strip_prefix(&root).with_context(|| {
        format!(
            "destination `{}` escapes root `{}`",
            target.display(),
            root.display()
        )
    })?;
    let mut current = root.clone();
    for component in relative.components() {
        current.push(component.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                bail!("destination path contains symlink `{}`", current.display());
            }
        }
    }
    Ok(())
}

fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)
        .with_context(|| format!("failed to create directory `{}`", target.display()))?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory `{}`", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .with_context(|| format!("failed to inspect `{}`", source_path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("refusing to copy symlink `{}`", source_path.display());
        }
        if metadata.is_dir() {
            copy_dir(&source_path, &target_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "failed to copy `{}` to `{}`",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        } else {
            bail!("unsupported file type `{}`", source_path.display());
        }
    }
    Ok(())
}

pub(crate) fn write_owner_metadata(metadata_path: &Path, action: &PlanAction) -> Result<()> {
    if let Some(parent) = metadata_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory `{}`", parent.display()))?;
    }
    let mut file = fs::File::create(metadata_path)
        .with_context(|| format!("failed to write metadata `{}`", metadata_path.display()))?;
    writeln!(file, "source={}", action.source.display())?;
    writeln!(file, "integrity={}", action.integrity)?;
    writeln!(
        file,
        "owners={}",
        action
            .owners
            .iter()
            .map(|owner| owner.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )?;
    Ok(())
}

pub(crate) fn installed_summary_lockfile_matches(manifest_dir: &Path) -> Result<bool> {
    let metadata_path = manifest_dir.join(".agentics/installed.yaml");
    if !metadata_path.is_file() {
        return Ok(true);
    }
    let expected = lockfile_hash(manifest_dir)?;
    let actual = read_yaml_string_field(&metadata_path, "lockfileHash")?;
    Ok(actual.as_deref() == Some(expected.as_str()))
}

fn read_yaml_string_field(path: &Path, key: &str) -> Result<Option<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read metadata `{}`", path.display()))?;
    let value: serde_yml::Value = serde_yml::from_str(&text)
        .with_context(|| format!("failed to parse metadata `{}`", path.display()))?;
    Ok(value
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yml::Value::String(key.to_string())))
        .and_then(|value| value.as_str())
        .map(str::to_string))
}

pub(crate) fn target_state(target: &Path, action: &PlanAction) -> Result<&'static str> {
    if !target.exists() {
        return Ok("missing");
    }
    let metadata_path = metadata_path_for(target, action.kind);
    if !metadata_path.is_file() {
        return Ok("unmanaged");
    }
    let current_integrity = hash_path(target)?;
    let expected_integrity = read_metadata_value(&metadata_path, "integrity")?;
    if expected_integrity.as_deref() != Some(current_integrity.as_str()) {
        return Ok("drifted");
    }
    if current_integrity != action.integrity {
        return Ok("outdated");
    }
    Ok("installed")
}

fn read_metadata_value(metadata_path: &Path, key: &str) -> Result<Option<String>> {
    if !metadata_path.is_file() {
        return Ok(None);
    }
    let contents = fs::read_to_string(metadata_path)
        .with_context(|| format!("failed to read metadata `{}`", metadata_path.display()))?;
    Ok(contents.lines().find_map(|line| {
        let (left, right) = line.split_once('=')?;
        (left == key).then(|| right.to_string())
    }))
}

pub(crate) fn metadata_path_for(target: &Path, kind: ActionKind) -> PathBuf {
    match kind {
        ActionKind::Directory => target.join(".agentics-owner"),
        ActionKind::File => target.with_extension("agentics-owner"),
    }
}
