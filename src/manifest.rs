use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use thiserror::Error;

use crate::{
    HarnessName, PiSkillRoot, ResourceType, SourceRef, is_supported_resource_for_harness,
    target_for,
};

pub(crate) const SUPPORTED_API_VERSION: &str = "agentics.dev/v1alpha1";
pub(crate) const SUPPORTED_KIND: &str = "AgenticsManifest";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) api_version: String,
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) policy: Policy,
    #[serde(default)]
    pub(crate) harnesses: Harnesses,
    #[serde(default)]
    pub(crate) catalogs: Vec<CatalogDeclaration>,
    #[serde(default)]
    pub(crate) install: Vec<InstallEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogDeclaration {
    pub(crate) name: String,
    pub(crate) source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    #[serde(default, rename = "requirePinnedGit")]
    pub(crate) require_pinned_git: bool,
    #[serde(default, rename = "requireResolvedLockCommit")]
    pub(crate) require_resolved_lock_commit: bool,
    #[serde(
        default = "default_allow_mutable_git_refs",
        rename = "allowMutableGitRefs"
    )]
    pub(crate) allow_mutable_git_refs: bool,
    #[serde(default, rename = "trustedSources")]
    pub(crate) trusted_sources: Vec<String>,
    #[serde(default, rename = "allowedExecutableResources")]
    pub(crate) allowed_executable_resources: Vec<String>,
    #[serde(default, rename = "allowGlobalInstall")]
    pub(crate) allow_global_install: bool,
}

fn default_allow_mutable_git_refs() -> bool {
    true
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            require_pinned_git: false,
            require_resolved_lock_commit: false,
            allow_mutable_git_refs: true,
            trusted_sources: Vec::new(),
            allowed_executable_resources: Vec::new(),
            allow_global_install: false,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Harnesses {
    #[serde(default)]
    pub(crate) claude: HarnessConfig,
    #[serde(default)]
    pub(crate) codex: HarnessConfig,
    #[serde(default)]
    pub(crate) pi: HarnessConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HarnessConfig {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default, rename = "skillRoot")]
    pub(crate) skill_root: PiSkillRoot,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallEntry {
    #[serde(rename = "type")]
    pub(crate) resource_type: ResourceType,
    pub(crate) name: String,
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) harnesses: Vec<HarnessName>,
    #[serde(default)]
    pub(crate) requires: Vec<String>,
    #[serde(default, rename = "managedInPlace")]
    pub(crate) managed_in_place: bool,
}

#[derive(Debug, Error)]
pub(crate) enum ValidationError {
    #[error("unsupported apiVersion `{actual}`; expected `{expected}`")]
    UnsupportedApiVersion {
        actual: String,
        expected: &'static str,
    },
    #[error("unsupported kind `{actual}`; expected `{expected}`")]
    UnsupportedKind {
        actual: String,
        expected: &'static str,
    },
    #[error("at least one harness must be enabled")]
    NoHarnessesEnabled,
    #[error("catalog #{index} has an empty name")]
    EmptyCatalogName { index: usize },
    #[error(
        "catalog `{name}` has invalid name; use lowercase letters, numbers, dots, underscores, and hyphens only"
    )]
    InvalidCatalogName { name: String },
    #[error("catalog `{name}` has an empty source")]
    EmptyCatalogSource { name: String },
    #[error("catalog `{name}` has invalid source: {message}")]
    InvalidCatalogSource { name: String, message: String },
    #[error("duplicate catalog `{name}`")]
    DuplicateCatalog { name: String },
    #[error("install entry #{index} has an empty name")]
    EmptyInstallName { index: usize },
    #[error(
        "install entry `{name}` has invalid resource name; use lowercase letters, numbers, dots, underscores, and hyphens only"
    )]
    InvalidInstallName { name: String },
    #[error("install entry `{name}` has an empty source")]
    EmptyInstallSource { name: String },
    #[error("install entry `{name}` has invalid source: {message}")]
    InvalidInstallSource { name: String, message: String },
    #[error("install entry `{name}` uses unpinned git source; specify #<rev>")]
    UnpinnedGitSource { name: String },
    #[error(
        "install entry `{name}` uses mutable git ref `{rev}`; set policy.allowMutableGitRefs: true or use a full commit SHA"
    )]
    MutableGitSource { name: String, rev: String },
    #[error(
        "install entry `{name}` uses unsupported high-risk resource type `{resource_type}` for MVP"
    )]
    UnsupportedHighRiskResource {
        name: String,
        resource_type: &'static str,
    },
    #[error("install entry `{name}` has unsupported context name; MVP supports only `agents`")]
    UnsupportedContextName { name: String },
    #[error(
        "install entry `{name}` type `{resource_type}` is unsupported for harness `{harness}` in MVP"
    )]
    UnsupportedHarnessResource {
        name: String,
        resource_type: &'static str,
        harness: &'static str,
    },
    #[error("install entry `{name}` targets disabled harness `{harness:?}`")]
    DisabledHarnessTarget { name: String, harness: HarnessName },
    #[error("duplicate install entry `{resource_type}` `{name}`")]
    DuplicateInstallEntry {
        resource_type: &'static str,
        name: String,
    },
    #[error("install entry `{name}` has duplicate harness target `{harness:?}`")]
    DuplicateHarnessTarget { name: String, harness: HarnessName },
    #[error("install entry `{name}` has invalid dependency `{dependency}`; expected <kind>:<name>")]
    InvalidDependency { name: String, dependency: String },
    #[error("install entry `{name}` has missing dependency `{dependency}`")]
    MissingDependency { name: String, dependency: String },
    #[error("dependency cycle detected involving `{name}`")]
    DependencyCycle { name: String },
    #[error("install entry `{name}` sets managedInPlace but source is not one of its targets")]
    ManagedInPlaceSourceMismatch { name: String },
}

impl Manifest {
    pub(crate) fn parse_yaml(input: &str) -> Result<Self> {
        serde_yml::from_str(input).context("failed to parse manifest YAML")
    }

    pub(crate) fn validate(&self) -> std::result::Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if self.api_version != SUPPORTED_API_VERSION {
            errors.push(ValidationError::UnsupportedApiVersion {
                actual: self.api_version.clone(),
                expected: SUPPORTED_API_VERSION,
            });
        }
        if self.kind != SUPPORTED_KIND {
            errors.push(ValidationError::UnsupportedKind {
                actual: self.kind.clone(),
                expected: SUPPORTED_KIND,
            });
        }

        let enabled = self.harnesses.enabled();
        if enabled.is_empty() {
            errors.push(ValidationError::NoHarnessesEnabled);
        }

        let mut catalog_names = BTreeSet::new();
        for (index, catalog) in self.catalogs.iter().enumerate() {
            if catalog.name.trim().is_empty() {
                errors.push(ValidationError::EmptyCatalogName { index });
            } else if !is_valid_resource_name(&catalog.name) {
                errors.push(ValidationError::InvalidCatalogName {
                    name: catalog.name.clone(),
                });
            } else if !catalog_names.insert(catalog.name.as_str()) {
                errors.push(ValidationError::DuplicateCatalog {
                    name: catalog.name.clone(),
                });
            }
            if catalog.source.trim().is_empty() {
                errors.push(ValidationError::EmptyCatalogSource {
                    name: catalog.name.clone(),
                });
            } else if let Err(error) = SourceRef::parse(&catalog.source) {
                errors.push(ValidationError::InvalidCatalogSource {
                    name: catalog.name.clone(),
                    message: error.to_string(),
                });
            }
        }

        let mut install_identities = BTreeSet::new();
        for (index, entry) in self.install.iter().enumerate() {
            if !install_identities.insert((entry.resource_type.as_str(), entry.name.as_str())) {
                errors.push(ValidationError::DuplicateInstallEntry {
                    resource_type: entry.resource_type.as_str(),
                    name: entry.name.clone(),
                });
            }

            if matches!(
                entry.resource_type,
                ResourceType::Extension | ResourceType::Package | ResourceType::Hook
            ) {
                errors.push(ValidationError::UnsupportedHighRiskResource {
                    name: entry.name.clone(),
                    resource_type: entry.resource_type.as_str(),
                });
            }
            if entry.resource_type == ResourceType::Context && entry.name != "agents" {
                errors.push(ValidationError::UnsupportedContextName {
                    name: entry.name.clone(),
                });
            }

            if entry.name.trim().is_empty() {
                errors.push(ValidationError::EmptyInstallName { index });
            } else if !is_valid_resource_name(&entry.name) {
                errors.push(ValidationError::InvalidInstallName {
                    name: entry.name.clone(),
                });
            }
            if entry.source.trim().is_empty() {
                errors.push(ValidationError::EmptyInstallSource {
                    name: entry.name.clone(),
                });
            } else {
                match SourceRef::parse(&entry.source) {
                    Ok(SourceRef::Git(git)) => {
                        let rev = git.rev.as_deref().unwrap_or_default();
                        if (self.policy.require_pinned_git
                            || self.policy.require_resolved_lock_commit)
                            && rev.is_empty()
                        {
                            errors.push(ValidationError::UnpinnedGitSource {
                                name: entry.name.clone(),
                            });
                        }
                        if !self.policy.allow_mutable_git_refs
                            && !rev.is_empty()
                            && !is_full_commit_sha(rev)
                        {
                            errors.push(ValidationError::MutableGitSource {
                                name: entry.name.clone(),
                                rev: rev.to_string(),
                            });
                        }
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(ValidationError::InvalidInstallSource {
                        name: entry.name.clone(),
                        message: error.to_string(),
                    }),
                }
            }

            let mut seen_dependencies = BTreeSet::new();
            for dependency in &entry.requires {
                match parse_dependency_ref(dependency) {
                    Some((resource_type, dependency_name)) => {
                        if !seen_dependencies.insert((resource_type.as_str(), dependency_name)) {
                            errors.push(ValidationError::InvalidDependency {
                                name: entry.name.clone(),
                                dependency: dependency.clone(),
                            });
                        }
                    }
                    None => errors.push(ValidationError::InvalidDependency {
                        name: entry.name.clone(),
                        dependency: dependency.clone(),
                    }),
                }
            }

            let mut seen_targets = BTreeSet::new();
            for harness in &entry.harnesses {
                if !seen_targets.insert(*harness) {
                    errors.push(ValidationError::DuplicateHarnessTarget {
                        name: entry.name.clone(),
                        harness: *harness,
                    });
                }
                if !enabled.contains(harness) {
                    errors.push(ValidationError::DisabledHarnessTarget {
                        name: entry.name.clone(),
                        harness: *harness,
                    });
                }
                if !is_supported_resource_for_harness(entry.resource_type, *harness) {
                    errors.push(ValidationError::UnsupportedHarnessResource {
                        name: entry.name.clone(),
                        resource_type: entry.resource_type.as_str(),
                        harness: harness.as_str(),
                    });
                }
            }

            if entry.managed_in_place {
                match SourceRef::parse(&entry.source) {
                    Ok(SourceRef::LocalPath(source)) => {
                        let source = normalize_relative_manifest_path(&source);
                        let owners: Vec<_> = if entry.harnesses.is_empty() {
                            enabled.iter().copied().collect()
                        } else {
                            entry.harnesses.clone()
                        };
                        let target_matches_source = owners.into_iter().any(|owner| {
                            target_for(self, entry.resource_type, owner, &entry.name)
                                .is_some_and(|target| target == source)
                        });
                        if !target_matches_source {
                            errors.push(ValidationError::ManagedInPlaceSourceMismatch {
                                name: entry.name.clone(),
                            });
                        }
                    }
                    _ => errors.push(ValidationError::ManagedInPlaceSourceMismatch {
                        name: entry.name.clone(),
                    }),
                }
            }
        }

        validate_dependencies(self, &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn validate_dependencies(manifest: &Manifest, errors: &mut Vec<ValidationError>) {
    let identities: BTreeSet<_> = manifest
        .install
        .iter()
        .map(|entry| (entry.resource_type.as_str(), entry.name.as_str()))
        .collect();

    for entry in &manifest.install {
        for dependency in &entry.requires {
            let Some((dependency_type, dependency_name)) = parse_dependency_ref(dependency) else {
                continue;
            };
            if !identities.contains(&(dependency_type.as_str(), dependency_name.as_str())) {
                errors.push(ValidationError::MissingDependency {
                    name: entry.name.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }

    if let Err(cycle_name) = sorted_install_indices(manifest) {
        errors.push(ValidationError::DependencyCycle { name: cycle_name });
    }
}

fn normalize_relative_manifest_path(path: &Path) -> PathBuf {
    path.components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect()
}

pub(crate) fn parse_dependency_ref(input: &str) -> Option<(ResourceType, String)> {
    let (kind, name) = input.split_once(':')?;
    if name.is_empty() || name.contains('/') || !is_valid_resource_name(name) {
        return None;
    }
    let resource_type = match kind {
        "skill" => ResourceType::Skill,
        "context" => ResourceType::Context,
        "prompt" => ResourceType::Prompt,
        "agent" => ResourceType::Agent,
        "extension" => ResourceType::Extension,
        "package" => ResourceType::Package,
        "hook" => ResourceType::Hook,
        _ => return None,
    };
    Some((resource_type, name.to_string()))
}

pub(crate) fn sorted_install_indices(
    manifest: &Manifest,
) -> std::result::Result<Vec<usize>, String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Visited,
    }

    fn visit(
        index: usize,
        manifest: &Manifest,
        by_id: &BTreeMap<(&'static str, &str), usize>,
        marks: &mut Vec<Option<Mark>>,
        sorted: &mut Vec<usize>,
    ) -> std::result::Result<(), String> {
        match marks[index] {
            Some(Mark::Visited) => return Ok(()),
            Some(Mark::Visiting) => return Err(manifest.install[index].name.clone()),
            None => {}
        }
        marks[index] = Some(Mark::Visiting);
        for dependency in &manifest.install[index].requires {
            let Some((dependency_type, dependency_name)) = parse_dependency_ref(dependency) else {
                continue;
            };
            if let Some(dependency_index) =
                by_id.get(&(dependency_type.as_str(), dependency_name.as_str()))
            {
                visit(*dependency_index, manifest, by_id, marks, sorted)?;
            }
        }
        marks[index] = Some(Mark::Visited);
        sorted.push(index);
        Ok(())
    }

    let by_id: BTreeMap<_, _> = manifest
        .install
        .iter()
        .enumerate()
        .map(|(index, entry)| ((entry.resource_type.as_str(), entry.name.as_str()), index))
        .collect();
    let mut marks = vec![None; manifest.install.len()];
    let mut sorted = Vec::with_capacity(manifest.install.len());
    for index in 0..manifest.install.len() {
        visit(index, manifest, &by_id, &mut marks, &mut sorted)?;
    }
    Ok(sorted)
}

pub(crate) fn is_valid_resource_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.contains("..")
}

fn is_full_commit_sha(rev: &str) -> bool {
    rev.len() == 40 && rev.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl Harnesses {
    pub(crate) fn enabled(&self) -> BTreeSet<HarnessName> {
        let mut enabled = BTreeSet::new();
        if self.claude.enabled {
            enabled.insert(HarnessName::Claude);
        }
        if self.codex.enabled {
            enabled.insert(HarnessName::Codex);
        }
        if self.pi.enabled {
            enabled.insert(HarnessName::Pi);
        }
        enabled
    }
}

pub(crate) fn load_valid_manifest(manifest_path: &Path) -> Result<Manifest> {
    let manifest_text = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read manifest `{}`", manifest_path.display()))?;
    let manifest = Manifest::parse_yaml(&manifest_text)?;
    if let Err(errors) = manifest.validate() {
        let message = errors
            .into_iter()
            .map(|error| format!("- {error}"))
            .collect::<Vec<_>>()
            .join("\n");
        bail!("manifest invalid: {}\n{}", manifest_path.display(), message);
    }
    Ok(manifest)
}
