use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEntry {
    state: String,
    target: String,
    source: String,
    owners: Vec<String>,
}

pub(crate) fn status(manifest_path: PathBuf, json: bool) -> Result<()> {
    let manifest = load_valid_manifest(&manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let plan = build_sync_plan(&manifest, manifest_dir, None)?;
    if plan.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No managed resources.");
        }
        return Ok(());
    }
    let installed_lockfile_matches = installed_summary_lockfile_matches(manifest_dir)?;
    let mut healthy = true;
    let mut entries = Vec::with_capacity(plan.len());
    for action in plan {
        let target = manifest_dir.join(&action.target);
        let mut state = target_state(&target, &action)?;
        if state == "installed" && !installed_lockfile_matches {
            state = "outdated";
        }
        if state != "installed" {
            healthy = false;
        }
        if json {
            entries.push(StatusEntry {
                state: state.to_string(),
                target: action.target.display().to_string(),
                source: action.display_source.clone(),
                owners: action
                    .owners
                    .iter()
                    .map(|owner| owner.as_str().to_string())
                    .collect(),
            });
        } else {
            println!("{} {}", state, action.target.display());
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    }
    if !healthy {
        std::process::exit(1);
    }
    Ok(())
}
