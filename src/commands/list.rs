use std::{collections::BTreeMap, path::PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListEntry {
    pub(crate) resource_type: String,
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) owners: Vec<String>,
    pub(crate) managed_in_place: bool,
}

pub(crate) fn list_resources(manifest_path: PathBuf, json: bool) -> Result<()> {
    let manifest = load_valid_manifest(&manifest_path)?;
    let entries = declared_resource_targets(&manifest);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if entries.is_empty() {
        println!("No managed resources.");
    } else {
        for entry in entries {
            println!(
                "{}:{} {} -> {} (owners: {})",
                entry.resource_type,
                entry.name,
                entry.source,
                entry.target,
                entry.owners.join(", ")
            );
        }
    }
    Ok(())
}

pub(crate) fn declared_resource_targets(manifest: &Manifest) -> Vec<ListEntry> {
    let enabled = manifest.harnesses.enabled();
    let mut by_target: BTreeMap<(ResourceType, &str, PathBuf), ListEntry> = BTreeMap::new();
    for entry in &manifest.install {
        let owners: Vec<_> = if entry.harnesses.is_empty() {
            enabled.iter().copied().collect()
        } else {
            entry.harnesses.clone()
        };
        for owner in owners {
            let Some(target) = target_for(manifest, entry.resource_type, owner, &entry.name) else {
                continue;
            };
            let key = (entry.resource_type, entry.name.as_str(), target.clone());
            let list_entry = by_target.entry(key).or_insert_with(|| ListEntry {
                resource_type: entry.resource_type.as_str().to_string(),
                name: entry.name.clone(),
                source: entry.source.clone(),
                target: target.display().to_string(),
                owners: Vec::new(),
                managed_in_place: entry.managed_in_place,
            });
            let owner = owner.as_str().to_string();
            if !list_entry.owners.contains(&owner) {
                list_entry.owners.push(owner);
                list_entry.owners.sort();
            }
        }
    }
    by_target.into_values().collect()
}
