# Refactor Plan: Split `src/main.rs`

`src/main.rs` is doing too much: CLI parsing, manifest schema, validation, source parsing, Git staging, lockfile generation, sync planning, file installation, doctor checks, docs, and command handlers all live in one file. The goal is to split it into focused modules while keeping behavior stable and preserving the existing test suite as the safety net.

## Target Shape

```text
src/
  main.rs              # parse CLI and dispatch commands only
  cli.rs               # clap structs/enums and command option types
  docs.rs              # built-in `agentics docs` text
  manifest.rs          # Manifest, Policy, Harnesses, InstallEntry, validation
  resources.rs         # ResourceType, HarnessName, target path mapping
  sources.rs           # SourceRef/GitSource parsing only
  validation.rs        # manifest/resource/source validation that spans modules
  policy.rs            # trusted source and executable warning policy helpers
  hash.rs              # hash_path, hash_file, file collection for integrity
  fsutil.rs            # atomic writes, path removal, safe path primitives
  lockfile.rs          # Lockfile/LockedResource, build/update/check/load
  git.rs               # Git staging/cache helpers and git command wrappers
  plan.rs              # PlanAction, ActionKind, sync/adopt/list plan construction
  install.rs           # copy/remove/write metadata/target state/installed summary
  commands/
    mod.rs
    init.rs
    doctor.rs
    status.rs
    list.rs
    adopt.rs
    sync.rs
    refresh.rs
    update.rs
    prune.rs
    completions.rs
```

Keep `anyhow::Result` as the shared error type for now. Avoid introducing a new abstraction layer during the split; first move code into modules with minimal logic changes.

## Module Boundaries

`main.rs`
- Owns `fn main()`.
- Calls `Cli::parse()` and dispatches to `commands::*`.
- Should be under 100 lines after the refactor.

`cli.rs`
- Contains `Cli`, `Command`, `DocsTopic`, `SyncOptions`, and `RefreshOptions`.
- Re-export command option structs as needed.
- Should not perform filesystem or Git work.

`manifest.rs`
- Contains manifest schema structs and schema-local validation:
  - `Manifest`
  - `Policy`
  - `Harnesses`
  - `HarnessConfig`
  - `PiSkillRoot`
  - `InstallEntry`
  - `CatalogDeclaration`
  - `ValidationError`
- Exposes `load_valid_manifest(path)`.
- Should not depend on Git, install planning, command handlers, or filesystem mutation.
- Keep only validation that can be performed from manifest fields alone.

`validation.rs`
- Contains validation that crosses module boundaries:
  - dependency sorting and validation helpers
  - `managedInPlace` target validation
  - source shape checks
  - frontmatter/resource warnings
  - executable content warning collection
- This prevents `manifest.rs` from depending on source parsing and target mapping in ways that create cycles.

`resources.rs`
- Contains resource and harness enums plus path mapping:
  - `ResourceType`
  - `HarnessName`
  - `PiSkillRoot`
  - `target_for`
  - `context_target`
  - `skill_target`
  - `prompt_target`
  - `agent_target`
  - harness support checks

`sources.rs`
- Contains source reference parsing:
  - `SourceRef`
  - `GitSource`
  - GitHub URL parsing
  - SCP-like Git source rejection
- Keep policy/trust checks out of this module.

`policy.rs`
- Contains policy interpretation helpers:
  - trusted Git source matching
  - source policy warnings
  - allowed executable resource checks
- Depends on `manifest::Policy` and parsed `sources`, but `sources` should not depend on policy.

`fsutil.rs`
- Contains reusable filesystem primitives:
  - `write_file_atomically`
  - `remove_path`
  - safe destination/path checks if they are not specific to plan installation
- Keep this separate from `install.rs` so lockfile, init, prune, and install code can share it without depending on installer semantics.

`lockfile.rs`
- Contains lockfile schema and update/check operations:
  - `Lockfile`
  - `LockedResource`
  - `build_lockfile`
  - `build_selective_lockfile`
  - `write_lockfile`
  - `load_lockfile`
  - `validate_lockfile`
  - `require_lockfile_for_sync`
  - `lockfile_hash`

`git.rs`
- Contains Git checkout/staging:
  - `StagedSource`
  - `GitStageCache`
  - `stage_git_source`
  - `stage_git_checkout`
  - `run_git`
  - `run_git_in`
  - `git_stdout_in`
  - `git_is_available`
- Should not depend on manifest or lockfile structs.

`plan.rs`
- Contains sync plan structures and plan creation:
  - `PlanAction`
  - `ActionKind`
  - `PlanEntry`
  - `StatusEntry` only if status stays plan-backed
  - `build_sync_plan`
  - `declared_resource_targets`
  - `targets_for_resource`
  - `plan_entry`
  - `dry_run_line`

`install.rs`
- Contains filesystem mutation and local metadata:
  - `InstallOutcome`
  - `InstalledSummary`
  - `InstalledSummaryEntry`
  - `install_action`
  - `check_write_preconditions`
  - `target_state`
  - `write_owner_metadata`
  - `write_installed_summary`
- `copy_dir`
- Uses `fsutil` for generic filesystem operations.
- Should stay focused on applying a `PlanAction` and maintaining ownership metadata.

`commands/*`
- Each command module should contain user-facing command flow and printing.
- Command modules can depend on lower-level modules, but lower-level modules should not depend on `commands`.

## Migration Sequence

1. Create `lib.rs` and move constants shared across modules.
   - Add `pub const SUPPORTED_API_VERSION`.
   - Add `pub const SUPPORTED_KIND`.
   - Keep `main.rs` dispatching through the library modules.

2. Extract CLI and docs.
   - Move clap definitions into `cli.rs`.
   - Move built-in docs strings into `docs.rs`.
   - Keep completions wired through `Cli::command()`.
   - This is low-risk and immediately shrinks `main.rs`.

3. Extract pure schema and enums.
   - Move manifest/resource structs and enums into `manifest.rs` and `resources.rs`.
   - Move `PiSkillRoot` with resource target mapping, or make target mapping take a small config value instead of `&Manifest`.
   - Make fields/functions `pub(crate)` only where needed.
   - Run `cargo fmt`, `cargo test -q`, and `cargo clippy --all-targets -- -D warnings`.

4. Extract source parsing, policy, validation, and hashing.
   - Move `SourceRef`, `GitSource`, and URL parsing into `sources.rs`.
   - Move trust normalization and policy warnings into `policy.rs`.
   - Move cross-module validation into `validation.rs`.
   - Move hashing helpers into `hash.rs`.
   - Keep function signatures unchanged where possible.
   - Verify all source URL, policy, and Git tests.

5. Extract generic filesystem helpers.
   - Move atomic writes and generic removal/path helpers into `fsutil.rs`.
   - Keep install-specific ownership metadata in `install.rs`.

6. Extract lockfile and Git staging.
   - Move lockfile schema/build/write/load logic into `lockfile.rs`.
   - Move Git staging/cache helpers into `git.rs`.
   - Move `action_kind_for` and `validate_source_shape` into `resources.rs` or `validation.rs`, not `plan.rs`.
   - Watch for circular dependencies between `lockfile`, `sources`, `git`, and `hash`; prefer passing resolved inputs instead of making modules know command flow.

7. Extract planning and installation.
   - Move `PlanAction`, `ActionKind`, plan building, target state, metadata, copy/remove, and installed summary logic.
   - Split read-only plan code from mutating install code so future dry-run work stays easy to reason about.

8. Extract command handlers.
   - Move each top-level command function into `commands/<name>.rs`.
   - Keep printing in command modules.
   - Keep reusable logic in lower-level modules.

9. Clean up visibility.
   - Replace broad `pub` with `pub(crate)`.
   - Remove dead helpers.
   - Keep tests black-box where possible; only add unit tests inside modules for truly private parsing or normalization behavior.

## Dependency Rules

Keep module dependencies mostly one-way:

```text
commands   -> manifest/validation/plan/install/lockfile/docs/git
validation -> manifest/resources/sources/policy/hash
plan       -> manifest/resources/sources/lockfile/hash
lockfile   -> manifest/resources/sources/hash/fsutil
install    -> resources/hash/fsutil
policy     -> manifest/sources
git        -> fsutil only if needed
```

Prefer command handlers as the orchestration boundary for environment-sensitive work:
- Commands resolve the manifest path, repo root, current directory, global flags, prompting, and non-interactive behavior.
- Commands perform Git staging/checkouts and pass resolved local paths plus lockfile data into planning.
- `plan.rs` should stay mostly read-only and deterministic.
- `lockfile.rs` should preserve lockfile shape and resolution semantics, but should avoid owning command flow or installer behavior.

Avoid:
- `manifest` depending on `commands`, `plan`, `install`, or `git`.
- `resources` depending on command behavior.
- `git` depending on lockfile or manifest structs.
- Filesystem mutation in modules named for parsing or validation.
- `sources` depending on `manifest` or `policy`.
- `lockfile` and `commands/init` depending on `install` for generic file writes.

## Acceptance Criteria

Phase gates:

- After each extraction phase, run `cargo fmt`, `cargo test -q`, and `cargo clippy --all-targets -- -D warnings`.
- Keep each phase independently shippable or easy to revert.
- Split lockfile extraction and Git staging extraction into separate verification points because lockfile compatibility and network/cache behavior fail differently.

Final gates:

- `src/main.rs` is under 100 lines.
- No module is over 700 lines after the first pass.
- `cargo fmt` passes.
- `cargo test -q` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- JSON and human-facing output contracts remain stable for:
  - `doctor --json`
  - `status --json`
  - `sync --dry-run --json`
  - human-facing `status` and `sync --dry-run`
- Lockfile YAML shape remains stable.
- Dry-run, status, validation, and docs commands do not write filesystem state.
- Local installer works against the built refactor artifact.
- Published installer works after pushing the rollout commit or tag:

```bash
curl -fsSL https://raw.githubusercontent.com/rymccue/agentics/master/install.sh | bash
```

- Existing repo smoke tests still pass in Conclave and Autosourcer:

```bash
agentics doctor --strict
agentics update --check
agentics sync --dry-run
agentics refresh --yes
```

## Refactor Guardrails

- Move code before rewriting code.
- Keep user-facing output stable unless a specific friction is being fixed.
- Commit after each compiling phase.
- Do not mix module extraction with behavior changes.
- Preserve `doctor --json`, `status --json`, and `sync --dry-run --json` schemas.
- Preserve lockfile YAML shape exactly.
- Preserve existing `.agentics` metadata format.

## Rollout Notes

- Prefer publishing a commit or tag first, then testing the installer against that exact artifact.
- Canary the release in Conclave and Autosourcer before asking the broader engineering org to adopt it.
- Keep a rollback path documented as reinstalling the previous published commit/tag or reverting the module split commit if behavior changes surface.
- Assign one owner to sign off on CLI output compatibility, lockfile compatibility, and installer behavior before broad rollout.
