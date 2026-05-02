use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

const SUPPORTED_API_VERSION: &str = "agentics.dev/v1alpha1";
const SUPPORTED_KIND: &str = "AgenticsManifest";

#[derive(Debug, Parser)]
#[command(
    name = "agentics",
    version,
    about = "Synchronize agentic resources across coding-agent harnesses"
)]
struct Cli {
    /// Path to the manifest file.
    #[arg(short, long, global = true, default_value = "agentics.yaml")]
    manifest: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a starter manifest in the current repository.
    Init {
        /// Comma-separated harnesses to enable, e.g. claude,codex,pi.
        #[arg(long)]
        harnesses: Option<String>,
        /// Catalog declaration to include, as name=source. May be repeated.
        #[arg(long)]
        catalog: Vec<String>,
        /// Add recommended agentics metadata patterns to .gitignore.
        #[arg(long)]
        gitignore: bool,
        /// Overwrite an existing manifest.
        #[arg(long)]
        force: bool,
    },
    /// Show synchronization state and drift.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Adopt matching existing targets by writing agentics ownership metadata.
    Adopt {
        /// Optional resource ID to adopt, such as skill:review.
        resource: Option<String>,
        /// Limit adoption to one enabled harness.
        #[arg(long)]
        harness: Option<HarnessName>,
        /// Print adoption actions without changing files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Synchronize resources into enabled harnesses.
    Sync {
        /// Print the write plan without changing files.
        #[arg(long)]
        dry_run: bool,
        /// Emit machine-readable JSON for dry-run plans.
        #[arg(long)]
        json: bool,
        /// Limit sync to one enabled harness.
        #[arg(long)]
        harness: Option<HarnessName>,
        /// Install into user-global harness locations instead of project-local targets.
        #[arg(long)]
        global: bool,
        /// Replace drifted resources that are already managed by agentics.
        #[arg(long)]
        force: bool,
        /// Assume yes for prompts that are safe to auto-confirm.
        #[arg(long)]
        yes: bool,
        /// Resolve and write the lockfile before applying sync.
        #[arg(long)]
        write_lock: bool,
        /// Fail instead of prompting for confirmation.
        #[arg(long)]
        non_interactive: bool,
    },
    /// Resolve the lockfile and synchronize resources.
    Refresh {
        /// Limit sync to one enabled harness.
        #[arg(long)]
        harness: Option<HarnessName>,
        /// Replace drifted resources that are already managed by agentics.
        #[arg(long)]
        force: bool,
        /// Assume yes for prompts that are safe to auto-confirm.
        #[arg(long)]
        yes: bool,
        /// Fail instead of prompting for confirmation.
        #[arg(long)]
        non_interactive: bool,
    },
    /// Resolve resources and update the lockfile.
    Update {
        /// Optional resource ID to refresh, such as skill:review.
        resource: Option<String>,
        /// Verify the lockfile is current without rewriting it.
        #[arg(long)]
        check: bool,
        /// Print the resolved lockfile without writing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate local configuration and environment.
    Doctor {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Treat warnings as failures.
        #[arg(long)]
        strict: bool,
    },
    /// List declared resources and target paths.
    List {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove managed targets no longer declared by the manifest.
    Prune {
        /// Print stale managed targets without removing them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print built-in documentation for agents and humans.
    Docs {
        /// Documentation topic to print.
        #[arg(value_enum, default_value_t = DocsTopic::Overview)]
        topic: DocsTopic,
    },
    /// Generate shell completions.
    Completions { shell: Shell },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DocsTopic {
    Overview,
    Migration,
    Ci,
    Manifest,
    Commands,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    api_version: String,
    kind: String,
    #[serde(default)]
    policy: Policy,
    #[serde(default)]
    harnesses: Harnesses,
    #[serde(default)]
    catalogs: Vec<CatalogDeclaration>,
    #[serde(default)]
    install: Vec<InstallEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogDeclaration {
    name: String,
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    #[serde(default, rename = "requirePinnedGit")]
    require_pinned_git: bool,
    #[serde(default, rename = "requireResolvedLockCommit")]
    require_resolved_lock_commit: bool,
    #[serde(
        default = "default_allow_mutable_git_refs",
        rename = "allowMutableGitRefs"
    )]
    allow_mutable_git_refs: bool,
    #[serde(default, rename = "trustedSources")]
    trusted_sources: Vec<String>,
    #[serde(default, rename = "allowedExecutableResources")]
    allowed_executable_resources: Vec<String>,
    #[serde(default, rename = "allowGlobalInstall")]
    allow_global_install: bool,
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
struct Harnesses {
    #[serde(default)]
    claude: HarnessConfig,
    #[serde(default)]
    codex: HarnessConfig,
    #[serde(default)]
    pi: HarnessConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default, rename = "skillRoot")]
    skill_root: PiSkillRoot,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum PiSkillRoot {
    #[default]
    Agents,
    Pi,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallEntry {
    #[serde(rename = "type")]
    resource_type: ResourceType,
    name: String,
    source: String,
    #[serde(default)]
    harnesses: Vec<HarnessName>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default, rename = "managedInPlace")]
    managed_in_place: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
enum ResourceType {
    Skill,
    Context,
    Prompt,
    Agent,
    Extension,
    Package,
    Hook,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ValueEnum, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
enum HarnessName {
    Claude,
    Codex,
    Pi,
}

impl ResourceType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Context => "context",
            Self::Prompt => "prompt",
            Self::Agent => "agent",
            Self::Extension => "extension",
            Self::Package => "package",
            Self::Hook => "hook",
        }
    }
}

impl HarnessName {
    fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Pi => "pi",
        }
    }
}

#[derive(Debug, Error)]
enum ValidationError {
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
    fn parse_yaml(input: &str) -> Result<Self> {
        serde_yml::from_str(input).context("failed to parse manifest YAML")
    }

    fn validate(&self) -> std::result::Result<(), Vec<ValidationError>> {
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

fn parse_dependency_ref(input: &str) -> Option<(ResourceType, String)> {
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

fn sorted_install_indices(manifest: &Manifest) -> std::result::Result<Vec<usize>, String> {
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

fn is_supported_resource_for_harness(resource_type: ResourceType, harness: HarnessName) -> bool {
    match resource_type {
        ResourceType::Skill | ResourceType::Context => true,
        ResourceType::Prompt => matches!(harness, HarnessName::Claude | HarnessName::Pi),
        ResourceType::Agent => matches!(harness, HarnessName::Claude),
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => false,
    }
}

fn is_valid_resource_name(name: &str) -> bool {
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
    fn enabled(&self) -> BTreeSet<HarnessName> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceRef {
    LocalPath(PathBuf),
    Git(GitSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitSource {
    repo: String,
    rev: Option<String>,
    subpath: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lockfile {
    api_version: String,
    kind: String,
    resources: Vec<LockedResource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LockedResource {
    #[serde(rename = "type")]
    resource_type: ResourceType,
    name: String,
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    integrity: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<String>,
}

impl Lockfile {
    fn find_resource(&self, name: &str, resource_type: ResourceType) -> Option<&LockedResource> {
        self.resources
            .iter()
            .find(|resource| resource.name == name && resource.resource_type == resource_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanAction {
    source: PathBuf,
    display_source: String,
    target: PathBuf,
    owners: Vec<HarnessName>,
    integrity: String,
    kind: ActionKind,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Directory,
    File,
}

impl SourceRef {
    fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        if input.is_empty() {
            bail!("source reference cannot be empty");
        }

        if let Some(rest) = input.strip_prefix("git:") {
            return parse_git_source(rest);
        }

        if looks_like_scp_git_source(input) {
            return parse_git_source(input);
        }

        if input.starts_with("https://github.com/")
            || input.starts_with("https://raw.githubusercontent.com/")
        {
            return parse_github_url(input);
        }

        if let Some(rest) = input.strip_prefix("file:") {
            return Ok(Self::LocalPath(PathBuf::from(rest)));
        }

        Ok(Self::LocalPath(PathBuf::from(input)))
    }
}

fn looks_like_scp_git_source(input: &str) -> bool {
    let Some((user_and_host, path)) = input.split_once(':') else {
        return false;
    };
    user_and_host.contains('@')
        && !path.is_empty()
        && !input[..input.find(':').unwrap_or(0)].contains('/')
}

fn parse_git_source(input: &str) -> Result<SourceRef> {
    let (repo, rev_and_subpath) = split_once(input, "#");
    let (rev, subpath) = rev_and_subpath
        .map(|value| split_once(value, "//"))
        .unwrap_or(("", None));
    if repo.is_empty() {
        bail!("git source must include a repository");
    }
    Ok(SourceRef::Git(GitSource {
        repo: repo.to_string(),
        rev: (!rev.is_empty()).then(|| rev.to_string()),
        subpath: subpath
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }))
}

fn parse_github_url(input: &str) -> Result<SourceRef> {
    let url = Url::parse(input).context("invalid GitHub URL")?;
    if url.fragment().is_some() {
        bail!("URL fragments are not supported in source references");
    }
    let host = url.host_str().unwrap_or_default();
    let segments: Vec<String> = url
        .path_segments()
        .map(|segments| segments.map(percent_decode_segment).collect())
        .unwrap_or_default();

    match host {
        "github.com" if segments.len() >= 5 && (segments[2] == "tree" || segments[2] == "blob") => {
            let repo = format!("https://github.com/{}/{}.git", segments[0], segments[1]);
            Ok(SourceRef::Git(GitSource {
                repo,
                rev: Some(segments[3].to_string()),
                subpath: Some(segments[4..].join("/")),
            }))
        }
        "github.com" if segments.len() == 2 => Ok(SourceRef::Git(GitSource {
            repo: format!("https://github.com/{}/{}.git", segments[0], segments[1]),
            rev: None,
            subpath: None,
        })),
        "raw.githubusercontent.com" if segments.len() >= 4 => Ok(SourceRef::Git(GitSource {
            repo: format!("https://github.com/{}/{}.git", segments[0], segments[1]),
            rev: Some(segments[2].to_string()),
            subpath: Some(segments[3..].join("/")),
        })),
        _ => bail!("unsupported GitHub URL shape"),
    }
}

fn percent_decode_segment(segment: &str) -> String {
    percent_decode_str(segment).decode_utf8_lossy().into_owned()
}

fn split_once<'a>(input: &'a str, delimiter: &str) -> (&'a str, Option<&'a str>) {
    match input.split_once(delimiter) {
        Some((left, right)) => (left, Some(right)),
        None => (input, None),
    }
}

fn hash_path(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect `{}`", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("refusing to hash symlink `{}`", path.display());
    }

    let mut hasher = Sha256::new();
    if metadata.is_file() {
        // Standalone file resources may be installed under a harness-specific filename
        // (for example a source prompt file can become `.claude/commands/<name>.md`).
        // Hash only content for files so status compares the managed payload rather
        // than the incidental source/target basename.
        hash_file(path, "", &mut hasher)?;
    } else if metadata.is_dir() {
        let mut files = Vec::new();
        collect_regular_files(path, path, &mut files)?;
        files.sort();
        for relative in files {
            hash_file(
                &path.join(&relative),
                &relative.to_string_lossy(),
                &mut hasher,
            )?;
        }
    } else {
        bail!("unsupported file type `{}`", path.display());
    }

    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn collect_regular_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(current)
        .with_context(|| format!("failed to read directory `{}`", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect `{}`", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("refusing to hash symlink `{}`", path.display());
        }
        if metadata.is_dir() {
            collect_regular_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path.strip_prefix(root)?.to_path_buf();
            if relative != Path::new(".agentics-owner") {
                files.push(relative);
            }
        } else {
            bail!("unsupported file type `{}`", path.display());
        }
    }

    Ok(())
}

fn hash_file(path: &Path, logical_name: &str, hasher: &mut Sha256) -> Result<()> {
    hasher.update(b"file\0");
    hasher.update(logical_name.as_bytes());
    hasher.update(b"\0");

    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open `{}`", path.display()))?;
    let mut buffer = [0_u8; 8192];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read `{}`", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    hasher.update(b"\0");
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .without_time()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            harnesses,
            catalog,
            gitignore,
            force,
        } => init(cli.manifest, harnesses, catalog, gitignore, force),
        Command::Status { json } => status(cli.manifest, json),
        Command::Adopt {
            resource,
            harness,
            dry_run,
        } => adopt(cli.manifest, resource, harness, dry_run),
        Command::Sync {
            dry_run,
            json,
            harness,
            global,
            force,
            yes,
            write_lock,
            non_interactive,
        } => sync(
            cli.manifest,
            SyncOptions {
                dry_run,
                json,
                harness,
                global,
                force,
                yes,
                write_lock,
                non_interactive,
            },
        ),
        Command::Refresh {
            harness,
            force,
            yes,
            non_interactive,
        } => refresh(
            cli.manifest,
            RefreshOptions {
                harness,
                force,
                yes,
                non_interactive,
            },
        ),
        Command::Update {
            resource,
            check,
            dry_run,
        } => update(cli.manifest, resource, check, dry_run),
        Command::Doctor { json, strict } => doctor(cli.manifest, json, strict),
        Command::List { json } => list_resources(cli.manifest, json),
        Command::Prune { dry_run } => prune(cli.manifest, dry_run),
        Command::Docs { topic } => docs(topic),
        Command::Completions { shell } => completions(shell),
    }
}

fn docs(topic: DocsTopic) -> Result<()> {
    println!("{}", docs_text(topic));
    Ok(())
}

fn docs_text(topic: DocsTopic) -> &'static str {
    match topic {
        DocsTopic::Overview => DOCS_OVERVIEW,
        DocsTopic::Migration => DOCS_MIGRATION,
        DocsTopic::Ci => DOCS_CI,
        DocsTopic::Manifest => DOCS_MANIFEST,
        DocsTopic::Commands => DOCS_COMMANDS,
    }
}

const DOCS_OVERVIEW: &str = r#"# agentics

agentics synchronizes repo-declared agent resources across harnesses.

Core files:
- agentics.yaml: desired skills, prompts, agents, and context
- agentics.lock.yaml: resolved source commits and integrity hashes
- .agentics/: local cache and installed-state metadata; do not commit
- *.agentics-owner: local per-target ownership metadata; do not commit

Common flow:
1. agentics doctor --strict
2. agentics update --check
3. agentics status
4. agentics sync --dry-run

Run `agentics docs commands`, `agentics docs manifest`, `agentics docs migration`, or `agentics docs ci` for focused guidance.
"#;

const DOCS_COMMANDS: &str = r#"# agentics commands

- agentics init --gitignore: add recommended metadata ignores.
- agentics doctor --strict: validate manifest, sources, metadata, and warnings.
- agentics update: resolve sources into agentics.lock.yaml.
- agentics update --check: fail if the lockfile is stale.
- agentics adopt: mark matching existing targets as managed without copying.
- agentics status: show installed, missing, unmanaged, drifted, or outdated targets.
- agentics sync --dry-run: preview current sync state and warnings.
- agentics sync --yes: install or update managed targets.
- agentics refresh --yes: update lockfile, then sync.
- agentics list: show declared resources and target paths.
- agentics prune --dry-run: preview managed targets no longer declared.
- agentics prune: remove stale managed targets.
"#;

const DOCS_MANIFEST: &str = r#"# agentics manifest

Recommended policy shape:

policy:
  requirePinnedGit: true
  trustedSources:
    - github.com/your-org/*
  allowedExecutableResources:
    - skill:trusted-script-skill

Use `managedInPlace: true` when the source path is intentionally also the target, such as AGENTS.md or .agents/skills/name.

Use GitHub `tree/main/...` sources only when you intentionally want `agentics update` or `agentics refresh` to pull the latest upstream source into the lockfile.

Commit:
- agentics.yaml
- agentics.lock.yaml
- installed shared skill directories if they are part of the repo

Do not commit:
- .agentics/
- *.agentics-owner
"#;

const DOCS_MIGRATION: &str = r#"# migrating an existing repo

1. Add metadata ignores:
   agentics init --gitignore

2. Create agentics.yaml and declare existing AGENTS.md, .claude, and .agents resources.

3. Mark existing target-owned resources with managedInPlace: true.

4. Resolve:
   agentics update

5. Adopt matching files:
   agentics adopt

6. Verify:
   agentics doctor --strict
   agentics status
   agentics sync --dry-run

7. Commit agentics.yaml, agentics.lock.yaml, and any newly installed shared resources.
"#;

const DOCS_CI: &str = r#"# CI

Recommended CI:

agentics doctor --strict
agentics update --check
agentics sync --dry-run

For repos tracking latest shared toolkit refs, developers should run:

agentics refresh --yes

Then commit the updated lockfile and installed shared resources.
"#;

fn completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    let name = command.get_name().to_string();
    generate(shell, &mut command, name, &mut std::io::stdout());
    Ok(())
}

fn init(
    manifest_path: PathBuf,
    harnesses: Option<String>,
    catalogs: Vec<String>,
    gitignore: bool,
    force: bool,
) -> Result<()> {
    if manifest_path.exists() && !force {
        if gitignore {
            let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
            ensure_gitignore_patterns(manifest_dir)?;
            println!("Updated {}", manifest_dir.join(".gitignore").display());
            return Ok(());
        }
        bail!("manifest already exists: {}", manifest_path.display());
    }
    if let Some(parent) = manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory `{}`", parent.display()))?;
    }
    fs::write(
        &manifest_path,
        starter_manifest(harnesses, catalogs)?.as_bytes(),
    )
    .with_context(|| format!("failed to write manifest `{}`", manifest_path.display()))?;
    println!("Created {}", manifest_path.display());
    if gitignore {
        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
        ensure_gitignore_patterns(manifest_dir)?;
        println!("Updated {}", manifest_dir.join(".gitignore").display());
    }
    Ok(())
}

fn ensure_gitignore_patterns(manifest_dir: &Path) -> Result<()> {
    let path = manifest_dir.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut lines_to_add = Vec::new();
    for pattern in ["/.agentics", "/.agentics-owner", "*.agentics-owner"] {
        if !gitignore_contains_pattern(&existing, pattern) {
            lines_to_add.push(pattern);
        }
    }
    if lines_to_add.is_empty() {
        return Ok(());
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    for pattern in lines_to_add {
        updated.push_str(pattern);
        updated.push('\n');
    }
    write_file_atomically(&path, updated.as_bytes())
}

fn gitignore_contains_pattern(contents: &str, pattern: &str) -> bool {
    contents.lines().map(str::trim).any(|line| line == pattern)
}

fn gitignore_warnings(manifest_dir: &Path) -> Vec<String> {
    let contents = fs::read_to_string(manifest_dir.join(".gitignore")).unwrap_or_default();
    ["/.agentics", "/.agentics-owner", "*.agentics-owner"]
        .into_iter()
        .filter(|pattern| !gitignore_contains_pattern(&contents, pattern))
        .map(|pattern| {
            format!(
                ".gitignore is missing `{pattern}`; run `agentics init --gitignore` to add recommended metadata ignores"
            )
        })
        .collect()
}

fn starter_manifest(harnesses: Option<String>, catalogs: Vec<String>) -> Result<String> {
    let enabled = if let Some(harnesses) = harnesses {
        let mut enabled = BTreeSet::new();
        for harness in harnesses
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match harness {
                "claude" => {
                    enabled.insert(HarnessName::Claude);
                }
                "codex" => {
                    enabled.insert(HarnessName::Codex);
                }
                "pi" => {
                    enabled.insert(HarnessName::Pi);
                }
                other => bail!("unsupported harness `{other}`"),
            }
        }
        if enabled.is_empty() {
            bail!("--harnesses must include at least one harness");
        }
        enabled
    } else {
        BTreeSet::from([HarnessName::Claude])
    };

    let mut manifest =
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n".to_string();
    for harness in [HarnessName::Claude, HarnessName::Codex, HarnessName::Pi] {
        if enabled.contains(&harness) {
            manifest.push_str(&format!("  {}:\n    enabled: true\n", harness.as_str()));
        }
    }
    if !catalogs.is_empty() {
        manifest.push_str("catalogs:\n");
        for catalog in catalogs {
            let Some((name, source)) = catalog.split_once('=') else {
                bail!("invalid catalog declaration `{catalog}`; expected name=source");
            };
            if !is_valid_resource_name(name) {
                bail!("invalid catalog name `{name}`");
            }
            if source.trim().is_empty() {
                bail!("catalog `{name}` has empty source");
            }
            manifest.push_str(&format!("  - name: {name}\n    source: {source}\n"));
        }
    }
    manifest.push_str("install: []\n");
    Ok(manifest)
}

#[derive(Debug, Serialize)]
struct StatusEntry {
    state: String,
    target: String,
    source: String,
    owners: Vec<String>,
}

fn status(manifest_path: PathBuf, json: bool) -> Result<()> {
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

fn adopt(
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListEntry {
    resource_type: String,
    name: String,
    source: String,
    target: String,
    owners: Vec<String>,
    managed_in_place: bool,
}

fn list_resources(manifest_path: PathBuf, json: bool) -> Result<()> {
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

fn declared_resource_targets(manifest: &Manifest) -> Vec<ListEntry> {
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

fn prune(manifest_path: PathBuf, dry_run: bool) -> Result<()> {
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

#[derive(Debug, Clone, Copy)]
struct SyncOptions {
    dry_run: bool,
    json: bool,
    harness: Option<HarnessName>,
    global: bool,
    force: bool,
    yes: bool,
    write_lock: bool,
    non_interactive: bool,
}

#[derive(Debug, Serialize)]
struct PlanEntry {
    state: String,
    source: String,
    target: String,
    owners: Vec<String>,
    kind: String,
    warnings: Vec<String>,
}

fn sync(manifest_path: PathBuf, options: SyncOptions) -> Result<()> {
    if options.json && !options.dry_run {
        bail!("--json requires --dry-run for sync plans");
    }
    let manifest = load_valid_manifest(&manifest_path)?;
    if options.global {
        if !manifest.policy.allow_global_install {
            bail!("policy blocked global installs; set policy.allowGlobalInstall: true to opt in");
        }
        bail!("global installs are not supported in MVP");
    }
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    if options.write_lock && !options.dry_run {
        if !options.yes {
            bail!("--write-lock requires --yes for mutating sync");
        }
        write_lockfile(&manifest, manifest_dir)?;
    } else if !options.dry_run {
        require_lockfile_for_sync(manifest_dir)?;
    }
    let plan = build_sync_plan(&manifest, manifest_dir, options.harness)?;
    if options.dry_run {
        for action in &plan {
            check_write_preconditions(manifest_dir, action, options.force)?;
        }
        if options.json {
            let entries = plan
                .iter()
                .map(|action| plan_entry_with_state(manifest_dir, action))
                .collect::<Result<Vec<_>>>()?;
            println!("{}", serde_json::to_string_pretty(&entries)?);
        } else if plan.is_empty() {
            println!("No actions planned.");
        } else {
            for action in plan {
                println!("{}", dry_run_line(manifest_dir, &action)?);
                if !action.warnings.is_empty() {
                    println!("  warnings:");
                    for warning in &action.warnings {
                        println!("    - {warning}");
                    }
                }
            }
        }
        return Ok(());
    }

    if options.non_interactive && !options.yes {
        if let Some(action) = plan.iter().find(|action| !action.warnings.is_empty()) {
            bail!(
                "policy blocked executable content or other warnings for `{}`; rerun with --yes if you trust this resource",
                action.target.display()
            );
        }
    }

    let mut applied_actions = Vec::with_capacity(plan.len());
    let mut changed = 0usize;
    let mut current = 0usize;
    for action in plan {
        match install_action(manifest_dir, &action, options.force)? {
            InstallOutcome::Changed => {
                changed += 1;
                println!("installed {}", action.target.display());
            }
            InstallOutcome::Current => {
                current += 1;
            }
        }
        applied_actions.push(action);
    }
    write_installed_summary(manifest_dir, &applied_actions)?;
    if changed == 0 && current > 0 {
        println!("All managed resources are already installed.");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct RefreshOptions {
    harness: Option<HarnessName>,
    force: bool,
    yes: bool,
    non_interactive: bool,
}

fn refresh(manifest_path: PathBuf, options: RefreshOptions) -> Result<()> {
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
            non_interactive: options.non_interactive,
        },
    )
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledSummary {
    lockfile_hash: String,
    installed: Vec<InstalledSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledSummaryEntry {
    target: String,
    source: String,
    integrity: String,
    kind: String,
    owners: Vec<String>,
}

fn write_installed_summary(manifest_dir: &Path, actions: &[PlanAction]) -> Result<()> {
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

fn lockfile_hash(manifest_dir: &Path) -> Result<String> {
    let path = manifest_dir.join("agentics.lock.yaml");
    let bytes =
        fs::read(&path).with_context(|| format!("failed to read lockfile `{}`", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn require_lockfile_for_sync(manifest_dir: &Path) -> Result<()> {
    let lockfile_path = manifest_dir.join("agentics.lock.yaml");
    if !lockfile_path.is_file() {
        bail!(
            "sync requires agentics.lock.yaml; run `agentics update` first, then rerun `agentics sync`"
        );
    }
    Ok(())
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

fn plan_entry_with_state(manifest_dir: &Path, action: &PlanAction) -> Result<PlanEntry> {
    let mut entry = plan_entry(action);
    entry.state = target_state(&manifest_dir.join(&action.target), action)?.to_string();
    Ok(entry)
}

fn dry_run_line(manifest_dir: &Path, action: &PlanAction) -> Result<String> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallOutcome {
    Changed,
    Current,
}

fn install_action(root: &Path, action: &PlanAction, force: bool) -> Result<InstallOutcome> {
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

fn check_write_preconditions(root: &Path, action: &PlanAction, force: bool) -> Result<()> {
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

fn ensure_safe_destination(root: &Path, target: &Path) -> Result<()> {
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

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect `{}`", path.display()))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
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

fn write_owner_metadata(metadata_path: &Path, action: &PlanAction) -> Result<()> {
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

fn installed_summary_lockfile_matches(manifest_dir: &Path) -> Result<bool> {
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

fn target_state(target: &Path, action: &PlanAction) -> Result<&'static str> {
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

fn metadata_path_for(target: &Path, kind: ActionKind) -> PathBuf {
    match kind {
        ActionKind::Directory => target.join(".agentics-owner"),
        ActionKind::File => target.with_extension("agentics-owner"),
    }
}

fn build_sync_plan(
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

fn load_lockfile(manifest_dir: &Path) -> Result<Option<Lockfile>> {
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

fn validate_lockfile(lockfile: &Lockfile) -> Result<()> {
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

fn source_path_for_sync(
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

fn action_kind_for(resource_type: ResourceType) -> Option<ActionKind> {
    match resource_type {
        ResourceType::Skill => Some(ActionKind::Directory),
        ResourceType::Context | ResourceType::Prompt | ResourceType::Agent => {
            Some(ActionKind::File)
        }
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => None,
    }
}

fn validate_source_shape(entry: &InstallEntry, source: &Path, kind: ActionKind) -> Result<()> {
    match kind {
        ActionKind::Directory => {
            let skill_file = source.join("SKILL.md");
            if !skill_file.is_file() {
                bail!("skill `{}` is missing {}", entry.name, skill_file.display());
            }
        }
        ActionKind::File => {
            if !source.is_file() {
                bail!(
                    "{} `{}` is missing file {}",
                    entry.resource_type.as_str(),
                    entry.name,
                    source.display()
                );
            }
        }
    }
    Ok(())
}

fn source_policy_warnings(manifest: &Manifest, source_ref: &SourceRef) -> Vec<String> {
    let SourceRef::Git(git) = source_ref else {
        return Vec::new();
    };
    if manifest.policy.trusted_sources.is_empty()
        || is_trusted_git_source(&git.repo, &manifest.policy.trusted_sources)
    {
        return Vec::new();
    }
    vec![format!("untrusted git source `{}`", git.repo)]
}

fn executable_content_warnings(path: &Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    if path.is_file() {
        if is_executable_like(path)? {
            warnings.push("contains executable content".to_string());
        }
        return Ok(warnings);
    }

    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    files.sort();
    for relative in files {
        let file = path.join(&relative);
        if is_executable_like(&file)? {
            warnings.push(format!(
                "contains executable content {}",
                relative.display()
            ));
        }
    }
    Ok(warnings)
}

fn filter_allowed_executable_warnings(
    manifest: &Manifest,
    entry: &InstallEntry,
    warnings: Vec<String>,
) -> Vec<String> {
    if !is_allowed_executable_resource(manifest, entry) {
        return warnings;
    }
    warnings
        .into_iter()
        .filter(|warning| !warning.starts_with("contains executable content"))
        .collect()
}

fn is_allowed_executable_resource(manifest: &Manifest, entry: &InstallEntry) -> bool {
    let id = format!("{}:{}", entry.resource_type.as_str(), entry.name);
    manifest
        .policy
        .allowed_executable_resources
        .iter()
        .any(|allowed| allowed == &id)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read directory `{}`", current.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read `{}`", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect `{}`", path.display()))?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(
                path.strip_prefix(root)
                    .with_context(|| format!("failed to relativize `{}`", path.display()))?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn is_executable_like(path: &Path) -> Result<bool> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if matches!(
        extension,
        "sh" | "bash" | "zsh" | "fish" | "py" | "rb" | "pl" | "js" | "ts" | "ps1" | "bat" | "cmd"
    ) {
        return Ok(true);
    }
    if path.file_name().and_then(|name| name.to_str()) == Some("package.json") {
        return Ok(true);
    }

    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to inspect executable content `{}`", path.display()))?;
    let mut prefix = [0_u8; 2];
    let bytes_read = file
        .read(&mut prefix)
        .with_context(|| format!("failed to read `{}`", path.display()))?;
    if bytes_read == 2 && prefix == *b"#!" {
        return Ok(true);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .with_context(|| format!("failed to inspect `{}`", path.display()))?
            .permissions()
            .mode();
        if mode & 0o111 != 0 {
            return Ok(true);
        }
    }

    Ok(false)
}

fn target_for(
    manifest: &Manifest,
    resource_type: ResourceType,
    harness: HarnessName,
    name: &str,
) -> Option<PathBuf> {
    match resource_type {
        ResourceType::Skill => Some(skill_target(manifest, harness, name)),
        ResourceType::Context => context_target(harness, name),
        ResourceType::Prompt => prompt_target(harness, name),
        ResourceType::Agent => agent_target(harness, name),
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => None,
    }
}

fn context_target(_harness: HarnessName, name: &str) -> Option<PathBuf> {
    (name == "agents").then(|| PathBuf::from("AGENTS.md"))
}

fn skill_target(manifest: &Manifest, harness: HarnessName, name: &str) -> PathBuf {
    match harness {
        HarnessName::Claude => PathBuf::from(".claude").join("skills").join(name),
        HarnessName::Codex => PathBuf::from(".agents").join("skills").join(name),
        HarnessName::Pi => match manifest.harnesses.pi.skill_root {
            PiSkillRoot::Agents => PathBuf::from(".agents").join("skills").join(name),
            PiSkillRoot::Pi => PathBuf::from(".pi").join("skills").join(name),
        },
    }
}

fn prompt_target(harness: HarnessName, name: &str) -> Option<PathBuf> {
    match harness {
        HarnessName::Claude => Some(
            PathBuf::from(".claude")
                .join("commands")
                .join(format!("{name}.md")),
        ),
        HarnessName::Pi => Some(
            PathBuf::from(".pi")
                .join("prompts")
                .join(format!("{name}.md")),
        ),
        HarnessName::Codex => None,
    }
}

fn agent_target(harness: HarnessName, name: &str) -> Option<PathBuf> {
    match harness {
        HarnessName::Claude => Some(
            PathBuf::from(".claude")
                .join("agents")
                .join(format!("{name}.md")),
        ),
        HarnessName::Codex | HarnessName::Pi => None,
    }
}

fn update(
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

fn write_lockfile(manifest: &Manifest, manifest_dir: &Path) -> Result<bool> {
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

fn write_file_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let temp_path = path.with_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!("{extension}.tmp"))
            .unwrap_or_else(|| "tmp".to_string()),
    );
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory `{}`", parent.display()))?;
    }
    fs::write(&temp_path, contents)
        .with_context(|| format!("failed to write temp file `{}`", temp_path.display()))?;
    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "failed to rename temp file `{}` to `{}`",
            temp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

fn build_selective_lockfile(
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

fn build_lockfile(manifest: &Manifest, manifest_dir: &Path) -> Result<Lockfile> {
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

struct StagedSource {
    path: PathBuf,
    commit: String,
}

struct GitStageCache {
    root: PathBuf,
    by_repo_ref: BTreeMap<String, (PathBuf, String)>,
}

impl GitStageCache {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            by_repo_ref: BTreeMap::new(),
        }
    }
}

fn stage_git_source(git: &GitSource, cache: &mut GitStageCache) -> Result<StagedSource> {
    let rev = git
        .rev
        .as_deref()
        .filter(|rev| !rev.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("unpinned git source `{}`; specify #<rev>", git.repo))?;
    let key = git_ref_cache_key(&git.repo, rev);
    let (clone_path, commit) = if let Some((clone_path, commit)) = cache.by_repo_ref.get(&key) {
        (clone_path.clone(), commit.clone())
    } else {
        let clone_path = stage_git_checkout(&git.repo, rev, &cache.root)?;
        let commit = git_stdout_in(&clone_path, ["rev-parse", "HEAD"])?;
        cache
            .by_repo_ref
            .insert(key, (clone_path.clone(), commit.clone()));
        (clone_path, commit)
    };
    let path = git
        .subpath
        .as_deref()
        .map(|subpath| safe_join_git_subpath(&clone_path, subpath))
        .transpose()?
        .unwrap_or_else(|| clone_path.clone());
    if !path.exists() {
        bail!("git source subpath does not exist: {}", path.display());
    }
    Ok(StagedSource { path, commit })
}

fn stage_git_checkout(repo: &str, rev: &str, staging_root: &Path) -> Result<PathBuf> {
    fs::create_dir_all(staging_root)
        .with_context(|| format!("failed to create staging root `{}`", staging_root.display()))?;
    let clone_path = staging_root
        .join(git_ref_cache_key(repo, rev))
        .with_extension("git-worktree");
    if clone_path.exists() {
        if git_stdout_in(&clone_path, ["rev-parse", "HEAD"])
            .ok()
            .as_deref()
            == Some(rev)
        {
            return Ok(clone_path);
        }
        remove_path(&clone_path)?;
    }
    run_git([
        "clone",
        "--quiet",
        "--no-checkout",
        repo,
        clone_path.to_string_lossy().as_ref(),
    ])?;
    run_git_in(&clone_path, ["checkout", "--quiet", rev])?;
    Ok(clone_path)
}

fn git_ref_cache_key(repo: &str, rev: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(repo.as_bytes());
    hasher.update(b"\0");
    hasher.update(rev.as_bytes());
    format!("git-{:x}", hasher.finalize())
}

fn safe_join_git_subpath(root: &Path, subpath: &str) -> Result<PathBuf> {
    let subpath = Path::new(subpath);
    if subpath.is_absolute()
        || subpath
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("git source subpath escapes checkout: {}", subpath.display());
    }
    Ok(root.join(subpath))
}

fn run_git<const N: usize>(args: [&str; N]) -> Result<()> {
    let status = ProcessCommand::new("git")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .status()
        .context("failed to execute git")?;
    if !status.success() {
        bail!("git command failed with status {status}");
    }
    Ok(())
}

fn run_git_in<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    let status = ProcessCommand::new("git")
        .current_dir(cwd)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .status()
        .context("failed to execute git")?;
    if !status.success() {
        bail!("git command failed with status {status}");
    }
    Ok(())
}

fn git_stdout_in<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .current_dir(cwd)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .output()
        .context("failed to execute git")?;
    if !output.status.success() {
        bail!("git command failed with status {}", output.status);
    }
    Ok(String::from_utf8(output.stdout)
        .context("git output was not utf-8")?
        .trim()
        .to_string())
}

fn load_valid_manifest(manifest_path: &Path) -> Result<Manifest> {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    valid: bool,
    manifest: String,
    git_available: bool,
    resources: Vec<DoctorResource>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DoctorResource {
    name: String,
    resource_type: String,
    source: String,
    integrity: Option<String>,
    warnings: Vec<String>,
}

fn doctor(manifest_path: PathBuf, json: bool, strict: bool) -> Result<()> {
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read manifest `{}`", manifest_path.display()))?;
    let git_available = git_is_available();
    let manifest = match Manifest::parse_yaml(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) if json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&DoctorReport {
                    valid: false,
                    manifest: manifest_path.display().to_string(),
                    git_available,
                    resources: Vec::new(),
                    warnings: Vec::new(),
                    errors: vec![error.to_string()],
                })?
            );
            std::process::exit(1);
        }
        Err(error) => return Err(error),
    };

    match manifest.validate() {
        Ok(()) => match validate_doctor_lockfile(&manifest_path)
            .and_then(|_| build_doctor_resources(&manifest, &manifest_path, json))
        {
            Ok(resources) => {
                let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
                let mut warnings = resources
                    .iter()
                    .flat_map(|resource| {
                        resource.warnings.iter().map(|warning| {
                            format!("{}:{}: {warning}", resource.resource_type, resource.name)
                        })
                    })
                    .collect::<Vec<_>>();
                warnings.extend(gitignore_warnings(manifest_dir));
                if strict && !warnings.is_empty() {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&DoctorReport {
                                valid: false,
                                manifest: manifest_path.display().to_string(),
                                git_available,
                                resources,
                                warnings: warnings.clone(),
                                errors: warnings,
                            })?
                        );
                        std::process::exit(1);
                    }
                    for warning in &warnings {
                        eprintln!("strict: {warning}");
                    }
                    bail!("strict doctor failed with {} warning(s)", warnings.len());
                }
                for warning in gitignore_warnings(manifest_dir) {
                    eprintln!("warning: {warning}");
                }
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&DoctorReport {
                            valid: true,
                            manifest: manifest_path.display().to_string(),
                            git_available,
                            resources,
                            warnings,
                            errors: Vec::new(),
                        })?
                    );
                } else {
                    if git_available {
                        println!("Git OK");
                    } else {
                        eprintln!(
                            "warning: git executable was not found; git sources cannot be updated"
                        );
                    }
                    println!("Manifest OK: {}", manifest_path.display());
                }
                Ok(())
            }
            Err(error) if json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&DoctorReport {
                        valid: false,
                        manifest: manifest_path.display().to_string(),
                        git_available,
                        resources: Vec::new(),
                        warnings: Vec::new(),
                        errors: vec![error.to_string()],
                    })?
                );
                std::process::exit(1);
            }
            Err(error) => Err(error),
        },
        Err(errors) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&DoctorReport {
                        valid: false,
                        manifest: manifest_path.display().to_string(),
                        git_available,
                        resources: Vec::new(),
                        warnings: Vec::new(),
                        errors: errors.into_iter().map(|error| error.to_string()).collect(),
                    })?
                );
            } else {
                eprintln!("Manifest invalid: {}", manifest_path.display());
                for error in errors {
                    eprintln!("- {error}");
                }
            }
            std::process::exit(1);
        }
    }
}

fn is_trusted_git_source(repo: &str, trusted_sources: &[String]) -> bool {
    let normalized_repo = normalize_git_repo_for_trust(repo);
    trusted_sources.iter().any(|pattern| {
        let pattern = pattern.trim().trim_end_matches(".git");
        if let Some(prefix) = pattern.strip_suffix('*') {
            normalized_repo.starts_with(prefix.trim_end_matches('*'))
        } else {
            normalized_repo == pattern
        }
    })
}

fn normalize_git_repo_for_trust(repo: &str) -> String {
    let without_scheme = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))
        .or_else(|| repo.strip_prefix("ssh://"))
        .unwrap_or(repo);
    let without_user = without_scheme
        .strip_prefix("git@")
        .unwrap_or(without_scheme)
        .replace(':', "/");
    without_user.trim_end_matches(".git").to_string()
}

fn resource_validation_warnings(entry: &InstallEntry, source: &Path) -> Result<Vec<String>> {
    match entry.resource_type {
        ResourceType::Skill => {
            let mut warnings = skill_metadata_warnings(&entry.name, &source.join("SKILL.md"))?;
            warnings.extend(nested_skill_warnings(&entry.name, source)?);
            Ok(warnings)
        }
        ResourceType::Prompt => markdown_frontmatter_warnings("prompt", &entry.name, source),
        ResourceType::Agent => markdown_frontmatter_warnings("agent", &entry.name, source),
        ResourceType::Context => Ok(Vec::new()),
        ResourceType::Extension | ResourceType::Package | ResourceType::Hook => Ok(Vec::new()),
    }
}

fn markdown_frontmatter_warnings(kind: &str, name: &str, path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read {kind} metadata `{}`", path.display()))?;
    let mut warnings = Vec::new();
    if let Some(frontmatter) = text.strip_prefix("---\n") {
        let Some((frontmatter, _body)) = frontmatter.split_once("\n---") else {
            warnings.push(format!("{kind} `{name}` has unterminated YAML frontmatter"));
            return Ok(warnings);
        };
        if let Err(error) = serde_yml::from_str::<serde_yml::Value>(frontmatter) {
            warnings.push(format!(
                "{kind} `{name}` has invalid YAML frontmatter: {error}"
            ));
        }
    }
    Ok(warnings)
}

fn nested_skill_warnings(name: &str, source: &Path) -> Result<Vec<String>> {
    let mut skill_files = Vec::new();
    collect_files(source, source, &mut skill_files)?;
    skill_files.sort();
    let nested = skill_files
        .into_iter()
        .filter(|relative| relative != Path::new("SKILL.md"))
        .filter(|relative| relative.file_name().and_then(|name| name.to_str()) == Some("SKILL.md"))
        .map(|relative| relative.display().to_string())
        .collect::<Vec<_>>();
    if nested.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![format!(
            "skill `{name}` contains nested skill files: {}; install the parent bundle when child skills depend on shared relative references",
            nested.join(", ")
        )])
    }
}

fn skill_metadata_warnings(name: &str, skill_file: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(skill_file)
        .with_context(|| format!("failed to read skill metadata `{}`", skill_file.display()))?;
    let mut warnings = Vec::new();
    let Some(frontmatter) = text.strip_prefix("---\n") else {
        warnings.push(format!(
            "skill `{name}` SKILL.md is missing YAML frontmatter"
        ));
        return Ok(warnings);
    };
    let Some((frontmatter, _body)) = frontmatter.split_once("\n---") else {
        warnings.push(format!(
            "skill `{name}` SKILL.md has unterminated YAML frontmatter"
        ));
        return Ok(warnings);
    };
    let metadata: serde_yml::Value = match serde_yml::from_str(frontmatter) {
        Ok(metadata) => metadata,
        Err(error) => {
            warnings.push(format!(
                "skill `{name}` SKILL.md has invalid YAML frontmatter: {error}"
            ));
            return Ok(warnings);
        }
    };
    let description = metadata
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yml::Value::String("description".to_string())))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if description.is_empty() {
        warnings.push(format!(
            "skill `{name}` SKILL.md frontmatter is missing non-empty description"
        ));
    }
    Ok(warnings)
}

fn validate_doctor_lockfile(manifest_path: &Path) -> Result<()> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    load_lockfile(manifest_dir)?;
    Ok(())
}

fn git_is_available() -> bool {
    ProcessCommand::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn build_doctor_resources(
    manifest: &Manifest,
    manifest_path: &Path,
    _json: bool,
) -> Result<Vec<DoctorResource>> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut resources = Vec::with_capacity(manifest.install.len());
    for entry in &manifest.install {
        let source_ref = SourceRef::parse(&entry.source)?;
        match source_ref {
            SourceRef::LocalPath(path) => {
                let absolute_or_relative = if path.is_absolute() {
                    path
                } else {
                    manifest_dir.join(path)
                };
                if !absolute_or_relative.exists() {
                    bail!(
                        "local source for `{}` is missing: {}",
                        entry.name,
                        absolute_or_relative.display()
                    );
                }
                if let Some(kind) = action_kind_for(entry.resource_type) {
                    validate_source_shape(entry, &absolute_or_relative, kind)?;
                }
                let mut warnings = resource_validation_warnings(entry, &absolute_or_relative)?;
                warnings.extend(executable_content_warnings(&absolute_or_relative)?);
                warnings = filter_allowed_executable_warnings(manifest, entry, warnings);
                for warning in &warnings {
                    eprintln!("warning: {warning}");
                }
                let integrity = hash_path(&absolute_or_relative)?;
                resources.push(DoctorResource {
                    name: entry.name.clone(),
                    resource_type: entry.resource_type.as_str().to_string(),
                    source: entry.source.clone(),
                    integrity: Some(integrity),
                    warnings,
                });
            }
            SourceRef::Git(git) => {
                let mut warnings = Vec::new();
                if git.rev.as_deref().unwrap_or_default().is_empty() {
                    warnings.push(format!(
                        "unpinned git source `{}`; `agentics update` will require #<rev>",
                        git.repo
                    ));
                }
                if !manifest.policy.trusted_sources.is_empty()
                    && !is_trusted_git_source(&git.repo, &manifest.policy.trusted_sources)
                {
                    warnings.push(format!("untrusted git source `{}`", git.repo));
                }
                for warning in &warnings {
                    eprintln!("warning: {warning}");
                }
                resources.push(DoctorResource {
                    name: entry.name.clone(),
                    resource_type: entry.resource_type.as_str().to_string(),
                    source: entry.source.clone(),
                    integrity: None,
                    warnings,
                });
            }
        }
    }
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MANIFEST: &str = r#"
apiVersion: agentics.dev/v1alpha1
kind: AgenticsManifest
harnesses:
  claude:
    enabled: true
  codex:
    enabled: true
install:
  - type: skill
    name: test-skill
    source: ./skills/test-skill
    harnesses: [claude, codex]
"#;

    #[test]
    fn parses_valid_manifest() {
        let manifest = Manifest::parse_yaml(VALID_MANIFEST).expect("manifest parses");
        assert_eq!(manifest.api_version, SUPPORTED_API_VERSION);
        assert_eq!(manifest.install.len(), 1);
        assert_eq!(manifest.install[0].resource_type, ResourceType::Skill);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn rejects_manifest_without_enabled_harnesses() {
        let manifest = Manifest::parse_yaml(
            r#"
apiVersion: agentics.dev/v1alpha1
kind: AgenticsManifest
install: []
"#,
        )
        .expect("manifest parses");

        let errors = manifest.validate().expect_err("manifest should be invalid");
        assert!(matches!(
            errors.as_slice(),
            [ValidationError::NoHarnessesEnabled]
        ));
    }

    #[test]
    fn rejects_disabled_target_harness() {
        let manifest = Manifest::parse_yaml(
            r#"
apiVersion: agentics.dev/v1alpha1
kind: AgenticsManifest
harnesses:
  claude:
    enabled: true
install:
  - type: skill
    name: pi-only
    source: ./skills/pi-only
    harnesses: [pi]
"#,
        )
        .expect("manifest parses");

        let errors = manifest.validate().expect_err("manifest should be invalid");
        assert!(errors.iter().any(|error| matches!(
            error,
            ValidationError::DisabledHarnessTarget {
                harness: HarnessName::Pi,
                ..
            }
        )));
    }

    #[test]
    fn parses_local_source_references() {
        assert_eq!(
            SourceRef::parse("./skills/review").expect("relative path parses"),
            SourceRef::LocalPath(PathBuf::from("./skills/review"))
        );
        assert_eq!(
            SourceRef::parse("file:/tmp/skills/review").expect("file path parses"),
            SourceRef::LocalPath(PathBuf::from("/tmp/skills/review"))
        );
    }

    #[test]
    fn parses_canonical_git_references() {
        assert_eq!(
            SourceRef::parse("git:https://github.com/acme/agentics.git#v1//skills/review")
                .expect("git source parses"),
            SourceRef::Git(GitSource {
                repo: "https://github.com/acme/agentics.git".to_string(),
                rev: Some("v1".to_string()),
                subpath: Some("skills/review".to_string()),
            })
        );
    }

    #[test]
    fn normalizes_github_browser_urls() {
        assert_eq!(
            SourceRef::parse("https://github.com/acme/agentics/tree/main/skills/review")
                .expect("github tree URL parses"),
            SourceRef::Git(GitSource {
                repo: "https://github.com/acme/agentics.git".to_string(),
                rev: Some("main".to_string()),
                subpath: Some("skills/review".to_string()),
            })
        );
    }

    #[test]
    fn parses_ssh_scp_like_git_references() {
        assert_eq!(
            SourceRef::parse("git@github.com:acme/agentics.git#main//skills/review")
                .expect("scp-like git source parses"),
            SourceRef::Git(GitSource {
                repo: "git@github.com:acme/agentics.git".to_string(),
                rev: Some("main".to_string()),
                subpath: Some("skills/review".to_string()),
            })
        );
    }

    #[test]
    fn normalizes_github_raw_urls() {
        assert_eq!(
            SourceRef::parse(
                "https://raw.githubusercontent.com/acme/agentics/main/skills/review/SKILL.md"
            )
            .expect("github raw URL parses"),
            SourceRef::Git(GitSource {
                repo: "https://github.com/acme/agentics.git".to_string(),
                rev: Some("main".to_string()),
                subpath: Some("skills/review/SKILL.md".to_string()),
            })
        );
    }

    #[test]
    fn directory_hash_is_deterministic_and_content_sensitive() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("nested")).expect("create nested dir");
        fs::write(temp.path().join("b.txt"), "beta").expect("write b");
        fs::write(temp.path().join("nested/a.txt"), "alpha").expect("write a");

        let first = hash_path(temp.path()).expect("first hash");
        let second = hash_path(temp.path()).expect("second hash");
        assert_eq!(first, second);

        fs::write(temp.path().join("nested/a.txt"), "changed").expect("change a");
        let changed = hash_path(temp.path()).expect("changed hash");
        assert_ne!(first, changed);
    }

    #[test]
    #[cfg(unix)]
    fn hashing_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("target.txt"), "target").expect("write target");
        symlink("target.txt", temp.path().join("link.txt")).expect("create symlink");

        let error = hash_path(temp.path()).expect_err("symlink should be rejected");
        assert!(error.to_string().contains("refusing to hash symlink"));
    }

    #[test]
    fn skill_targets_map_to_harness_locations() {
        let manifest = Manifest::parse_yaml(VALID_MANIFEST).expect("manifest parses");
        assert_eq!(
            skill_target(&manifest, HarnessName::Claude, "review"),
            PathBuf::from(".claude/skills/review")
        );
        assert_eq!(
            skill_target(&manifest, HarnessName::Codex, "review"),
            PathBuf::from(".agents/skills/review")
        );
        assert_eq!(
            skill_target(&manifest, HarnessName::Pi, "review"),
            PathBuf::from(".agents/skills/review")
        );
    }

    #[test]
    fn resource_name_validation_blocks_path_escapes() {
        assert!(is_valid_resource_name("review-agent_1.2"));
        assert!(!is_valid_resource_name("../escape"));
        assert!(!is_valid_resource_name("Upper"));
        assert!(!is_valid_resource_name(".hidden"));
        assert!(!is_valid_resource_name("two..dots"));
    }
}
