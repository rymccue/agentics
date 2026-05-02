use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn sync_rejects_duplicate_lockfile_entries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");
    std::fs::write(
        temp.path().join("agentics.lock.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsLock\nresources:\n  - type: skill\n    name: review\n    source: ./skills/review\n    integrity: sha256:one\n  - type: skill\n    name: review\n    source: ./skills/review\n    integrity: sha256:two\n",
    )
    .expect("write lockfile");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate lockfile entry"));
}
