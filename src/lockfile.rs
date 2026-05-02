use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    manifest::{SUPPORTED_API_VERSION, parse_dependency_ref},
    *,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Lockfile {
    api_version: String,
    kind: String,
    resources: Vec<LockedResource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LockedResource {
    #[serde(rename = "type")]
    pub(crate) resource_type: ResourceType,
    pub(crate) name: String,
    pub(crate) source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) integrity: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) dependencies: Vec<String>,
}

impl Lockfile {
    pub(crate) fn find_resource(
        &self,
        name: &str,
        resource_type: ResourceType,
    ) -> Option<&LockedResource> {
        self.resources
            .iter()
            .find(|resource| resource.name == name && resource.resource_type == resource_type)
    }
}

pub(crate) fn lockfile_hash(manifest_dir: &Path) -> Result<String> {
    let path = manifest_dir.join("agentics.lock.yaml");
    let bytes =
        fs::read(&path).with_context(|| format!("failed to read lockfile `{}`", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub(crate) fn require_lockfile_for_sync(manifest_dir: &Path) -> Result<()> {
    let lockfile_path = manifest_dir.join("agentics.lock.yaml");
    if !lockfile_path.is_file() {
        bail!(
            "sync requires agentics.lock.yaml; run `agentics update` first, then rerun `agentics sync`"
        );
    }
    Ok(())
}
pub(crate) fn load_lockfile(manifest_dir: &Path) -> Result<Option<Lockfile>> {
    let path = manifest_dir.join("agentics.lock.yaml");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lockfile `{}`", path.display()))?;
    let lockfile: Lockfile = serde_yml::from_str(&text).context("failed to parse lockfile")?;
    if lockfile.api_version != SUPPORTED_API_VERSION || lockfile.kind != "AgenticsLock" {
        bail!("unsupported lockfile `{}`", path.display());
    }
    validate_lockfile(&lockfile)?;
    Ok(Some(lockfile))
}

pub(crate) fn validate_lockfile(lockfile: &Lockfile) -> Result<()> {
    let mut seen = BTreeSet::new();
    for resource in &lockfile.resources {
        if !seen.insert((resource.resource_type.as_str(), resource.name.as_str())) {
            bail!(
                "duplicate lockfile entry `{}` `{}`",
                resource.resource_type.as_str(),
                resource.name
            );
        }
    }
    Ok(())
}

pub(crate) fn source_path_for_sync(
    entry: &InstallEntry,
    source_ref: &SourceRef,
    manifest_dir: &Path,
    lockfile: Option<&Lockfile>,
) -> Result<PathBuf> {
    match source_ref {
        SourceRef::LocalPath(path) => Ok(path.clone()),
        SourceRef::Git(git) => {
            let locked = lockfile
                .and_then(|lockfile| lockfile.find_resource(&entry.name, entry.resource_type))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "git source `{}` requires agentics update before sync",
                        entry.name
                    )
                })?;
            let commit = locked.commit.as_deref().ok_or_else(|| {
                anyhow::anyhow!("git source `{}` is missing lockfile commit", entry.name)
            })?;
            let staged = stage_git_checkout(
                git.repo.as_str(),
                commit,
                &manifest_dir.join(".agentics/cache/sources"),
            )?;
            let staged_commit =
                git_stdout_in(&staged, ["rev-parse", "HEAD"]).with_context(|| {
                    format!("failed to inspect staged git source `{}`", staged.display())
                })?;
            if staged_commit != commit {
                bail!(
                    "staged git source `{}` does not match lockfile commit",
                    entry.name
                );
            }
            Ok(git
                .subpath
                .as_deref()
                .map(|subpath| staged.join(subpath))
                .unwrap_or(staged))
        }
    }
}
pub(crate) fn write_lockfile(manifest: &Manifest, manifest_dir: &Path) -> Result<bool> {
    let lockfile = build_lockfile(manifest, manifest_dir)?;
    let lockfile_path = manifest_dir.join("agentics.lock.yaml");
    let yaml = serde_yml::to_string(&lockfile).context("failed to serialize lockfile")?;
    if fs::read(&lockfile_path)
        .map(|existing| existing == yaml.as_bytes())
        .unwrap_or(false)
    {
        println!("Lockfile unchanged: {}", lockfile_path.display());
        return Ok(false);
    }
    write_file_atomically(&lockfile_path, yaml.as_bytes())?;
    println!("Updated {}", lockfile_path.display());
    Ok(true)
}

pub(crate) fn build_selective_lockfile(
    manifest: &Manifest,
    manifest_dir: &Path,
    resource_id: &str,
) -> Result<Lockfile> {
    let (resource_type, name) = parse_dependency_ref(resource_id)
        .ok_or_else(|| anyhow::anyhow!("invalid resource id `{resource_id}`"))?;
    if !manifest
        .install
        .iter()
        .any(|entry| entry.resource_type == resource_type && entry.name == name)
    {
        bail!("resource `{resource_id}` is not declared in manifest");
    }
    let existing = load_lockfile(manifest_dir)?
        .ok_or_else(|| anyhow::anyhow!("selective update requires existing agentics.lock.yaml"))?;
    let refreshed = build_lockfile(manifest, manifest_dir)?;
    let mut resources = existing.resources;
    for refreshed_resource in refreshed.resources {
        if refreshed_resource.resource_type == resource_type && refreshed_resource.name == name {
            if let Some(existing_resource) = resources
                .iter_mut()
                .find(|resource| resource.resource_type == resource_type && resource.name == name)
            {
                *existing_resource = refreshed_resource;
            } else {
                resources.push(refreshed_resource);
            }
            return Ok(Lockfile {
                api_version: SUPPORTED_API_VERSION.to_string(),
                kind: "AgenticsLock".to_string(),
                resources,
            });
        }
    }
    bail!("resource `{resource_id}` was not resolved")
}

pub(crate) fn build_lockfile(manifest: &Manifest, manifest_dir: &Path) -> Result<Lockfile> {
    let mut resources = Vec::with_capacity(manifest.install.len());
    let staging_root = manifest_dir.join(".agentics/cache/update");
    let mut git_cache = GitStageCache::new(staging_root);
    let sorted_indices = sorted_install_indices(manifest)
        .map_err(|name| anyhow::anyhow!("dependency cycle detected involving `{name}`"))?;
    for index in sorted_indices {
        let entry = &manifest.install[index];
        let source_ref = SourceRef::parse(&entry.source)?;
        let (commit, integrity) = match source_ref {
            SourceRef::LocalPath(path) => {
                let path = if path.is_absolute() {
                    path
                } else {
                    manifest_dir.join(path)
                };
                if !path.exists() {
                    bail!(
                        "local source for `{}` is missing: {}",
                        entry.name,
                        path.display()
                    );
                }
                if let Some(kind) = action_kind_for(entry.resource_type) {
                    validate_source_shape(entry, &path, kind)?;
                }
                (None, Some(hash_path(&path)?))
            }
            SourceRef::Git(git) => {
                let staged = stage_git_source(&git, &mut git_cache)?;
                if let Some(kind) = action_kind_for(entry.resource_type) {
                    validate_source_shape(entry, &staged.path, kind)?;
                }
                (Some(staged.commit), Some(hash_path(&staged.path)?))
            }
        };
        resources.push(LockedResource {
            resource_type: entry.resource_type,
            name: entry.name.clone(),
            source: entry.source.clone(),
            commit,
            integrity,
            dependencies: entry.requires.clone(),
        });
    }

    Ok(Lockfile {
        api_version: SUPPORTED_API_VERSION.to_string(),
        kind: "AgenticsLock".to_string(),
        resources,
    })
}
