use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn status_reports_outdated_when_installed_metadata_lockfile_hash_differs() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();
    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();

    let metadata_path = temp.path().join(".agentics/installed.yaml");
    let metadata = std::fs::read_to_string(&metadata_path).unwrap();
    std::fs::write(
        &metadata_path,
        metadata.replace("lockfileHash: sha256:", "lockfileHash: sha256:stale"),
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .failure()
        .stdout(predicate::str::contains("outdated .claude/skills/review"));
}
