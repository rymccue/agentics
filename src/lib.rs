use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;
mod docs;
mod fsutil;
mod git;
mod hash;
mod install;
mod lockfile;
mod manifest;
mod plan;
mod policy;
mod resources;
mod sources;
mod validation;

use cli::{Cli, Command};
use commands::{
    adopt::adopt,
    completions::completions,
    doctor::doctor,
    init::init,
    list::list_resources,
    prune::prune,
    refresh::{RefreshOptions, refresh},
    status::status,
    sync::{SyncOptions, sync},
    update::update,
};
use docs::docs;
use fsutil::{remove_path, write_file_atomically};
use git::{GitStageCache, git_stdout_in, stage_git_checkout, stage_git_source};
use hash::hash_path;
use install::{
    InstallOutcome, InstalledSummary, InstalledSummaryEntry, adopt_existing_matching_targets,
    check_write_preconditions, ensure_safe_destination, install_action,
    installed_summary_lockfile_matches, metadata_path_for, target_state, write_installed_summary,
    write_owner_metadata,
};
use lockfile::{
    build_lockfile, build_selective_lockfile, load_lockfile, lockfile_hash,
    require_lockfile_for_sync, source_path_for_sync, write_lockfile,
};
use manifest::{InstallEntry, Manifest, load_valid_manifest, sorted_install_indices};
use plan::{PlanAction, build_sync_plan, dry_run_line, plan_entry_with_state};
use policy::is_trusted_git_source;
#[cfg(test)]
use resources::skill_target;
use resources::{
    ActionKind, HarnessName, PiSkillRoot, ResourceType, action_kind_for,
    is_supported_resource_for_harness, target_for,
};
use sources::{GitSource, SourceRef};
use validation::{
    collect_files, executable_content_warnings, filter_allowed_executable_warnings,
    source_policy_warnings, validate_source_shape,
};

pub fn run() -> Result<()> {
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
            adopt_existing,
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
                adopt_existing,
                non_interactive,
            },
        ),
        Command::Refresh {
            harness,
            force,
            yes,
            adopt_existing,
            non_interactive,
        } => refresh(
            cli.manifest,
            RefreshOptions {
                harness,
                force,
                yes,
                adopt_existing,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{SUPPORTED_API_VERSION, ValidationError, is_valid_resource_name};
    use std::{fs, path::PathBuf};

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
