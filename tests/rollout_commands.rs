use assert_cmd::Command;
use predicates::prelude::*;

fn write_recommended_gitignore(root: &std::path::Path) {
    std::fs::write(
        root.join(".gitignore"),
        "/.agentics\n/.agentics-owner\n*.agentics-owner\n",
    )
    .expect("write gitignore");
}

fn write_skill(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join(format!("skills/{name}"));
    std::fs::create_dir_all(&dir).expect("create skill");
    std::fs::write(dir.join("SKILL.md"), body).expect("write skill");
}

#[test]
fn init_gitignore_adds_recommended_metadata_ignores() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(temp.path().join(".gitignore"), "target/\n").expect("write gitignore");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["init", "--gitignore"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated .gitignore"));

    let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();
    assert!(gitignore.contains("target/"));
    assert!(gitignore.contains("/.agentics"));
    assert!(gitignore.contains("*.agentics-owner"));
}

#[test]
fn doctor_strict_fails_on_warnings() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_recommended_gitignore(temp.path());
    write_skill(temp.path(), "review", "# Review\n");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["doctor", "--strict"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("strict doctor failed"));
}

#[test]
fn doctor_strict_fails_when_metadata_ignores_are_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_skill(
        temp.path(),
        "review",
        "---\ndescription: Review skill.\n---\n# Review\n",
    );
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["doctor", "--strict"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(".gitignore is missing"));
}

#[test]
fn allowed_executable_resources_suppress_executable_warnings() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_recommended_gitignore(temp.path());
    write_skill(
        temp.path(),
        "ops",
        "---\ndescription: Ops skill.\n---\n# Ops\n",
    );
    std::fs::create_dir_all(temp.path().join("skills/ops/scripts")).expect("create scripts");
    std::fs::write(temp.path().join("skills/ops/scripts/run.sh"), "#!/bin/sh\n")
        .expect("write script");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  allowedExecutableResources:\n    - skill:ops\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: ops\n    source: ./skills/ops\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["doctor", "--strict"])
        .assert()
        .success()
        .stderr(predicate::str::contains("contains executable content").not());

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("contains executable content").not());
}

#[test]
fn list_shows_declared_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  codex:\n    enabled: true\n  pi:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [codex, pi]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("skill:review")
                .and(predicate::str::contains(".agents/skills/review"))
                .and(predicate::str::contains("owners: codex, pi")),
        );
}

#[test]
fn refresh_updates_lockfile_and_syncs() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_skill(
        temp.path(),
        "review",
        "---\ndescription: Review skill.\n---\n# Review\n",
    );
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["refresh"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Updated agentics.lock.yaml")
                .and(predicate::str::contains("installed .claude/skills/review")),
        );

    assert!(temp.path().join("agentics.lock.yaml").is_file());
    assert!(temp.path().join(".claude/skills/review/SKILL.md").is_file());
}

#[test]
fn refresh_is_quiet_when_everything_is_current() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_skill(
        temp.path(),
        "review",
        "---\ndescription: Review skill.\n---\n# Review\n",
    );
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("refresh")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("refresh")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Lockfile unchanged: agentics.lock.yaml")
                .and(predicate::str::contains(
                    "All managed resources are already installed.",
                ))
                .and(predicate::str::contains("installed .claude/skills/review").not()),
        );
}

#[test]
fn docs_command_prints_local_agent_guidance() {
    let temp = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["docs", "migration"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("migrating an existing repo")
                .and(predicate::str::contains("agentics adopt")),
        );
}

#[test]
fn prune_removes_installed_targets_not_declared_by_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_skill(
        temp.path(),
        "review",
        "---\ndescription: Review skill.\n---\n# Review\n",
    );
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["refresh"])
        .assert()
        .success();

    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall: []\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("prune")
        .assert()
        .success()
        .stdout(predicate::str::contains("pruned .claude/skills/review"));

    assert!(!temp.path().join(".claude/skills/review").exists());
}
