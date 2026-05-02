use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlanAction {
    pub(crate) source: PathBuf,
    pub(crate) display_source: String,
    pub(crate) target: PathBuf,
    pub(crate) owners: Vec<HarnessName>,
    pub(crate) integrity: String,
    pub(crate) kind: ActionKind,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PlanEntry {
    pub(crate) state: String,
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) owners: Vec<String>,
    pub(crate) kind: String,
    pub(crate) warnings: Vec<String>,
}

fn plan_entry(action: &PlanAction) -> PlanEntry {
    PlanEntry {
        state: "planned".to_string(),
        source: action.display_source.clone(),
        target: action.target.display().to_string(),
        owners: action
            .owners
            .iter()
            .map(|owner| owner.as_str().to_string())
            .collect(),
        kind: match action.kind {
            ActionKind::Directory => "directory".to_string(),
            ActionKind::File => "file".to_string(),
        },
        warnings: action.warnings.clone(),
    }
}

pub(crate) fn plan_entry_with_state(manifest_dir: &Path, action: &PlanAction) -> Result<PlanEntry> {
    let mut entry = plan_entry(action);
    entry.state = target_state(&manifest_dir.join(&action.target), action)?.to_string();
    Ok(entry)
}

pub(crate) fn dry_run_line(manifest_dir: &Path, action: &PlanAction) -> Result<String> {
    let state = target_state(&manifest_dir.join(&action.target), action)?;
    let verb = match state {
        "installed" => "installed",
        "missing" => "would install",
        "outdated" => "would update",
        "drifted" => "would replace drifted",
        "unmanaged" => "unmanaged",
        other => other,
    };
    let owners = action
        .owners
        .iter()
        .map(|owner| owner.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "{verb} {} from {} (owners: {})",
        action.target.display(),
        action.display_source,
        owners
    ))
}

pub(crate) fn build_sync_plan(
    manifest: &Manifest,
    manifest_dir: &Path,
    harness_filter: Option<HarnessName>,
) -> Result<Vec<PlanAction>> {
    let enabled = manifest.harnesses.enabled();
    if let Some(harness) = harness_filter {
        if !enabled.contains(&harness) {
            bail!("harness `{}` is not enabled", harness.as_str());
        }
    }
    let lockfile = load_lockfile(manifest_dir)?;
    let sorted_indices = sorted_install_indices(manifest)
        .map_err(|name| anyhow::anyhow!("dependency cycle detected involving `{name}`"))?;
    let mut plan: Vec<PlanAction> = Vec::new();
    let mut target_indices: BTreeMap<PathBuf, usize> = BTreeMap::new();

    for index in sorted_indices {
        let entry = &manifest.install[index];
        let Some(kind) = action_kind_for(entry.resource_type) else {
            continue;
        };
        let source_ref = SourceRef::parse(&entry.source)?;
        let source = source_path_for_sync(entry, &source_ref, manifest_dir, lockfile.as_ref())?;
        let source_for_validation = if source.is_absolute() {
            source.clone()
        } else {
            manifest_dir.join(&source)
        };
        validate_source_shape(entry, &source_for_validation, kind)?;
        let mut warnings = filter_allowed_executable_warnings(
            manifest,
            entry,
            executable_content_warnings(&source_for_validation)?,
        );
        warnings.extend(source_policy_warnings(manifest, &source_ref));
        let integrity = hash_path(&source_for_validation)?;
        if let Some(locked_resource) = lockfile
            .as_ref()
            .and_then(|lockfile| lockfile.find_resource(&entry.name, entry.resource_type))
        {
            if locked_resource.source != entry.source {
                bail!("lockfile source mismatch for `{}`", entry.name);
            }
            if let Some(locked) = locked_resource.integrity.as_deref() {
                if locked != integrity {
                    bail!("locked integrity mismatch for `{}`", entry.name);
                }
            }
        }

        let mut owners: Vec<_> = if entry.harnesses.is_empty() {
            enabled.iter().copied().collect()
        } else {
            entry.harnesses.clone()
        };
        if let Some(harness) = harness_filter {
            owners.retain(|owner| *owner == harness);
        }

        for owner in owners {
            let Some(target) = target_for(manifest, entry.resource_type, owner, &entry.name) else {
                continue;
            };
            if let Some(action_index) = target_indices.get(&target).copied() {
                let action = &mut plan[action_index];
                if action.source != source
                    || action.display_source != entry.source
                    || action.integrity != integrity
                    || action.kind != kind
                    || action.warnings != warnings
                {
                    // Mark a conflict for validation below while preserving deterministic output.
                    action.owners.clear();
                } else if !action.owners.contains(&owner) {
                    action.owners.push(owner);
                    action.owners.sort();
                }
            } else {
                target_indices.insert(target.clone(), plan.len());
                plan.push(PlanAction {
                    source: source.clone(),
                    display_source: entry.source.clone(),
                    target,
                    owners: vec![owner],
                    integrity: integrity.clone(),
                    kind,
                    warnings: warnings.clone(),
                });
            }
        }
    }

    if let Some(conflict) = plan.iter().find(|action| action.owners.is_empty()) {
        bail!("target conflict for `{}`", conflict.target.display());
    }
    Ok(plan)
}
