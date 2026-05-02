use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::HarnessName;

#[derive(Debug, Parser)]
#[command(
    name = "agentics",
    version,
    about = "Synchronize agentic resources across coding-agent harnesses"
)]
pub(crate) struct Cli {
    /// Path to the manifest file.
    #[arg(short, long, global = true, default_value = "agentics.yaml")]
    pub(crate) manifest: PathBuf,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
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
        /// Adopt existing matching targets before syncing.
        #[arg(long)]
        adopt_existing: bool,
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
        /// Adopt existing matching targets before syncing.
        #[arg(long)]
        adopt_existing: bool,
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
pub(crate) enum DocsTopic {
    Overview,
    Migration,
    Ci,
    Manifest,
    Commands,
}
