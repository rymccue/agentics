# agentics

`agentics` is a Rust CLI for synchronizing repo-declared agentic resources across coding-agent harnesses.

A repository declares its desired skills, prompts, agents, and related resources in `agentics.yaml`. The CLI validates the manifest, resolves sources into `agentics.lock.yaml`, and safely installs supported resources into harness-native locations.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/rymccue/agentics/master/install.sh | bash
```

The installer uses `cargo install --git`, so it requires Git and Rust/Cargo. After install, run `agentics docs` for local CLI guidance.

## MVP support

Supported resources:

- `skill`
  - Claude: `.claude/skills/<name>`
  - Codex: `.agents/skills/<name>`
  - Pi: `.agents/skills/<name>` by default, or `.pi/skills/<name>` with `harnesses.pi.skillRoot: pi`
- `context`
  - `name: agents` installs shared context to `AGENTS.md`
- `prompt`
  - Claude: `.claude/commands/<name>.md`
  - Pi: `.pi/prompts/<name>.md`
  - Codex: unsupported in MVP
- `agent`
  - Claude: `.claude/agents/<name>.md`
  - Codex/Pi: unsupported in MVP

High-risk executable/config-mutating resources (`package`, `extension`, `hook`) are intentionally rejected in the MVP.

## Commands

```bash
agentics init
agentics init --gitignore
agentics init --harnesses claude,pi
agentics init --catalog team=git:https://github.com/myorg/catalog.git#v1//catalog.yaml
agentics init --force
agentics doctor
agentics doctor --strict
agentics doctor --json
agentics list
agentics list --json
agentics update
agentics update skill:review
agentics update --dry-run
agentics update --check
agentics adopt
agentics adopt skill:review
agentics adopt --harness claude
agentics sync --dry-run
agentics sync --dry-run --json
agentics sync --harness claude --dry-run
agentics sync
agentics sync --write-lock --yes
agentics refresh
agentics docs
agentics docs commands
agentics docs manifest
agentics docs migration
agentics docs ci
agentics prune --dry-run
agentics prune
agentics status
agentics status --json
agentics completions bash
```

Safety flags for `sync`:

```bash
agentics sync --harness claude
agentics sync --force
agentics sync --yes
agentics sync --non-interactive
agentics sync --global
```

`--harness <claude|codex|pi>` limits sync planning and writes to a single enabled harness. It is useful for previewing or applying one adapter’s target paths at a time.

`--force` replaces drifted resources already managed by agentics. It never overwrites unmanaged targets.

`agentics sync` is lockfile-driven and requires `agentics.lock.yaml`. Run `agentics update` first, or use `agentics sync --write-lock --yes` to intentionally resolve and write the lockfile as part of a mutating sync. `sync --dry-run` remains side-effect free and can preview plans before a lockfile exists.

`agentics update --dry-run` prints the resolved lockfile YAML without writing it. `agentics update <type:name>` selectively refreshes one resource in an existing lockfile while preserving other locked entries.

`agentics adopt` writes agentics ownership metadata for existing targets whose content already matches the manifest source and lockfile integrity. It does not copy files or overwrite content. Use it when adding `agentics.yaml` to a repo that already has `.claude/`, `.agents/`, or `AGENTS.md` resources in place.

`agentics refresh` is shorthand for resolving the latest lockfile and then syncing it. Use it when a manifest intentionally tracks mutable upstream refs such as GitHub `tree/main` toolkit paths.

`agentics docs` prints built-in documentation from the installed CLI, so agents and engineers can get rollout, migration, CI, manifest, and command guidance without opening a browser or finding this README.

`agentics list` prints declared resources and their harness target paths. `agentics prune` removes managed targets that were previously installed but are no longer declared in the manifest.

`agentics doctor --strict` treats warnings as failures. It is intended for CI and organization-wide rollout checks.

`--non-interactive` never prompts. It blocks plans with warnings such as executable content or untrusted Git sources unless `--yes` is also provided. `--global` is recognized but policy-gated and not implemented for MVP writes.

## Manifest example

```yaml
apiVersion: agentics.dev/v1alpha1
kind: AgenticsManifest
policy:
  requirePinnedGit: true
  allowMutableGitRefs: true
  trustedSources:
    - github.com/myorg/*
  allowedExecutableResources:
    - skill:atlassian
  allowGlobalInstall: false
catalogs:
  - name: team
    source: git:https://github.com/myorg/catalog.git#v1//catalog.yaml
harnesses:
  claude:
    enabled: true
  codex:
    enabled: true
  pi:
    enabled: true
    skillRoot: agents
install:
  - type: skill
    name: review
    source: ./skills/review
    harnesses: [claude, codex, pi]
    requires:
      - prompt:summarize
  - type: context
    name: agents
    source: ./AGENTS.md
    managedInPlace: true
    harnesses: [claude, codex, pi]
  - type: prompt
    name: summarize
    source: ./prompts/summarize.md
    harnesses: [claude, pi]
  - type: agent
    name: reviewer
    source: ./agents/reviewer.md
    harnesses: [claude]
```

## Source references

Supported source forms:

- Local relative path: `./skills/review`
- Local file URI: `file:/absolute/path/to/skill`
- Canonical Git: `git:https://github.com/org/repo.git#<rev>//path/in/repo`
- GitHub browser URLs such as `https://github.com/org/repo/tree/main/skills/review`
- GitHub raw URLs such as `https://raw.githubusercontent.com/org/repo/main/skills/review/SKILL.md`

Git sources must include a ref, either with canonical `#<rev>` syntax or a GitHub URL such as `https://github.com/org/repo/tree/main/path`. `agentics update` resolves that ref to an exact commit in `agentics.lock.yaml`. Mutable refs such as `main` are allowed by default so a later `agentics update` can intentionally pull the latest upstream content; set `policy.allowMutableGitRefs: false` to require full 40-character commit SHAs. `agentics doctor` warns on Git sources with no ref by default; set `policy.requirePinnedGit: true` or `policy.requireResolvedLockCommit: true` to make missing refs a manifest validation error. GitHub browser/raw URL fragments such as `#L1` are rejected because source references must resolve to resources, not UI anchors.

`policy.trustedSources` is an allowlist for Git source trust warnings. Entries support simple exact matches and trailing wildcards, for example `github.com/myorg/repo` or `github.com/myorg/*`.

`policy.allowedExecutableResources` suppresses executable-content warnings for specific trusted resources. Use resource IDs such as `skill:atlassian`, and keep this list small and reviewed.

Set `managedInPlace: true` on a local resource when the source path is intentionally the same file or directory as one of its harness targets, such as `AGENTS.md` or `.agents/skills/<name>`. The flag is validated so accidental self-targeting is explicit.

Catalog declarations are parsed and validated so manifests can declare future catalog inputs. Full catalog resource resolution/search is still deferred beyond the current MVP path.

## Safety behavior

- Validates manifest shape, resource names, dependencies, and cycles.
- Validates existing `agentics.lock.yaml` schema/kind during `doctor`.
- Reports Git availability in `doctor`/`doctor --json` because Git is required for remote source updates.
- Topologically orders dependency installs before dependents.
- Rejects duplicate install identities.
- Rejects unsupported high-risk resource types.
- Rejects symlinks and non-regular files in staged/copied resources.
- Warns about executable content, script-like files, package manifests, malformed frontmatter, and untrusted Git sources.
- Warns when recommended local metadata ignores are missing from `.gitignore`; run `agentics init --gitignore` to add them.
- Refuses to overwrite unmanaged targets.
- Writes per-target ownership/integrity metadata and a local `.agentics/installed.yaml` summary.
- Detects missing, installed, drifted, unmanaged, and outdated targets.
- Verifies lockfile source and integrity before mutating sync.

## Installed metadata

Successful mutating syncs write local machine state under `.agentics/installed.yaml`, plus lightweight per-target owner files used for drift protection. The installed summary includes a `lockfileHash` so local installs can be tied back to the exact `agentics.lock.yaml` used during sync; `agentics status` reports otherwise-installed targets as `outdated` when that hash no longer matches. `.agentics/` is ignored by git; commit `agentics.yaml` and `agentics.lock.yaml`, not installed metadata.

## Examples

A runnable example manifest and resource set lives in `examples/basic`:

```bash
cd examples/basic
agentics doctor
agentics update --check
agentics sync --dry-run
```

## Recommended repo pattern

- Commit `agentics.yaml`.
- Commit `agentics.lock.yaml`.
- Ignore local metadata:

```gitignore
/.agentics
/.agentics-owner
*.agentics-owner
```

- Use `managedInPlace: true` when the source path is intentionally also the harness target.
- Use GitHub `tree/main/...` sources only for approved shared toolkits where "latest on update" is the intended behavior.
- Use `policy.trustedSources` for approved remote source owners.
- Use `policy.allowedExecutableResources` only for reviewed script-backed skills.

## CI

Recommended CI check:

```bash
agentics doctor --strict
agentics update --check
agentics sync --dry-run
```

For repos that intentionally track latest toolkit refs, developers refresh locally and commit the lockfile:

```bash
agentics refresh --yes
git add agentics.yaml agentics.lock.yaml .claude .agents
```

## Migrating an existing repo

1. Add recommended ignores:

```bash
agentics init --gitignore
```

2. Create `agentics.yaml` declaring existing `.claude/`, `.agents/`, and `AGENTS.md` resources. Add `managedInPlace: true` when the source is already the target.

3. Resolve the lockfile:

```bash
agentics update
```

4. Adopt existing matching files:

```bash
agentics adopt
```

5. Confirm state and preview future sync behavior:

```bash
agentics status
agentics sync --dry-run
```

6. Commit manifest, lockfile, and any newly installed shared skills. Do not commit `.agentics/` or `*.agentics-owner`.

## Development

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```
