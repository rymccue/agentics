use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};

use crate::{manifest::parse_dependency_ref, *};

pub(crate) fn adopt(
    manifest_path: PathBuf,
    resource: Option<String>,
    harness: Option<HarnessName>,
    dry_run: bool,
) -> Result<()> {
    let manifest = load_valid_manifest(&manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    require_lockfile_for_sync(manifest_dir)?;
    let mut plan = build_sync_plan(&manifest, manifest_dir, harness)?;
    if let Some(resource_id) = resource.as_deref() {
        let (resource_type, name) = parse_dependency_ref(resource_id)
            .ok_or_else(|| anyhow::anyhow!("invalid resource id `{resource_id}`"))?;
        let targets = targets_for_resource(&manifest, resource_type, &name, harness)?;
        if targets.is_empty() {
            bail!("resource `{resource_id}` has no targets for selected harnesses");
        }
        plan.retain(|action| targets.contains(&action.target));
        if plan.is_empty() {
            bail!("resource `{resource_id}` was not planned");
        }
    }

    let mut adopted = Vec::new();
    for action in &plan {
        let target = manifest_dir.join(&action.target);
        if !target.exists() {
            bail!("cannot adopt missing target `{}`", action.target.display());
        }
        let current_integrity = hash_path(&target)?;
        if current_integrity != action.integrity {
            bail!(
                "cannot adopt `{}` because target content does not match manifest source",
                action.target.display()
            );
        }
        let metadata_path = metadata_path_for(&target, action.kind);
        if metadata_path.is_file() {
            println!("already managed {}", action.target.display());
            continue;
        }
        if dry_run {
            println!("adopt {}", action.target.display());
        } else {
            write_owner_metadata(&metadata_path, action)?;
            println!("adopted {}", action.target.display());
        }
        adopted.push(action.clone());
    }

    if !dry_run && !adopted.is_empty() && all_plan_targets_installed(manifest_dir, &plan)? {
        write_installed_summary(manifest_dir, &plan)?;
    }
    Ok(())
}

fn all_plan_targets_installed(manifest_dir: &Path, plan: &[PlanAction]) -> Result<bool> {
    for action in plan {
        let target = manifest_dir.join(&action.target);
        if target_state(&target, action)? != "installed" {
            return Ok(false);
        }
    }
    Ok(true)
}

fn targets_for_resource(
    manifest: &Manifest,
    resource_type: ResourceType,
    name: &str,
    harness_filter: Option<HarnessName>,
) -> Result<BTreeSet<PathBuf>> {
    let entry = manifest
        .install
        .iter()
        .find(|entry| entry.resource_type == resource_type && entry.name == name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "resource `{}` `{}` is not declared",
                resource_type.as_str(),
                name
            )
        })?;
    let enabled = manifest.harnesses.enabled();
    let mut owners: Vec<_> = if entry.harnesses.is_empty() {
        enabled.iter().copied().collect()
    } else {
        entry.harnesses.clone()
    };
    if let Some(harness) = harness_filter {
        if !enabled.contains(&harness) {
            bail!("harness `{}` is not enabled", harness.as_str());
        }
        owners.retain(|owner| *owner == harness);
    }
    Ok(owners
        .into_iter()
        .filter_map(|owner| target_for(manifest, resource_type, owner, name))
        .collect())
}
