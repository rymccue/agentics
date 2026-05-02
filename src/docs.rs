use anyhow::Result;

use crate::cli::DocsTopic;

pub(crate) fn docs(topic: DocsTopic) -> Result<()> {
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
