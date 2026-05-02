use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SyncOptions {
    pub(crate) dry_run: bool,
    pub(crate) json: bool,
    pub(crate) harness: Option<HarnessName>,
    pub(crate) global: bool,
    pub(crate) force: bool,
    pub(crate) yes: bool,
    pub(crate) write_lock: bool,
    pub(crate) non_interactive: bool,
}

pub(crate) fn sync(manifest_path: PathBuf, options: SyncOptions) -> Result<()> {
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
