use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_rejects_git_source_missing_from_lockfile() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:https://example.com/repo.git#abc123//skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");
    std::fs::write(
        temp.path().join("agentics.lock.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsLock\nresources: []\n",
    )
    .expect("write lockfile");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "requires agentics update before sync",
        ));
}

#[test]
fn sync_rejects_lockfile_source_mismatch() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("skills/review")).expect("create skill");
    std::fs::write(temp.path().join("skills/review/SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");
    std::fs::write(
        temp.path().join("agentics.lock.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsLock\nresources:\n  - type: skill\n    name: review\n    source: ./other\n    commit: null\n    integrity: sha256:bad\n",
    )
    .expect("write lockfile");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("lockfile source mismatch"));
}
