use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn update_writes_lockfile_with_local_resource_integrity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").expect("write skill");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    let mut command = Command::cargo_bin("agentics").expect("binary exists");
    command
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated"));

    let lockfile = std::fs::read_to_string(temp.path().join("agentics.lock.yaml"))
        .expect("lockfile was written");
    assert!(lockfile.contains("kind: AgenticsLock"));
    assert!(lockfile.contains("name: review"));
    assert!(lockfile.contains("integrity: sha256:"));
}
