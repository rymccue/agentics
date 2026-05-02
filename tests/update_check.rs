use assert_cmd::Command;
use predicates::prelude::*;

fn write_manifest_and_skill(root: &std::path::Path, contents: &str) {
    let skill_dir = root.join("skills/review");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), contents).expect("write skill");
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");
}

#[test]
fn update_check_succeeds_when_lockfile_is_current() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_manifest_and_skill(temp.path(), "# Review\n");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["update", "--check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Lockfile OK"));
}

#[test]
fn update_check_fails_when_lockfile_is_stale() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_manifest_and_skill(temp.path(), "# Review\n");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    std::fs::write(temp.path().join("skills/review/SKILL.md"), "# Changed\n")
        .expect("change skill");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["update", "--check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("lockfile is out of date"));
}
