use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn update_resolves_local_git_source_commit_and_integrity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("resource-repo");
    std::fs::create_dir_all(repo.join("skills/review")).expect("create skill");
    std::fs::write(repo.join("skills/review/SKILL.md"), "# Review\n").expect("write skill");
    Command::new("git")
        .current_dir(&repo)
        .args(["init"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(&repo)
        .args(["config", "user.email", "test@example.com"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(&repo)
        .args(["config", "user.name", "Test User"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(&repo)
        .args(["add", "."])
        .assert()
        .success();
    Command::new("git")
        .current_dir(&repo)
        .args(["commit", "-m", "initial"])
        .assert()
        .success();
    let commit = String::from_utf8(
        Command::new("git")
            .current_dir(&repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse")
            .stdout,
    )
    .expect("utf8");
    let commit = commit.trim();

    std::fs::write(
        temp.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{}#{}//skills/review\n    harnesses: [claude]\n",
            repo.display(), commit
        ),
    )
    .expect("write manifest");

    let mut update = Command::cargo_bin("agentics").expect("binary exists");
    update
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    let lockfile =
        std::fs::read_to_string(temp.path().join("agentics.lock.yaml")).expect("lockfile written");
    assert!(lockfile.contains(commit));
    assert!(lockfile.contains("integrity: sha256:"));
}

#[test]
fn update_rejects_unpinned_git_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:https://github.com/acme/resources.git//skills/review\n    harnesses: [claude]\n",
    )
    .expect("write manifest");

    let mut update = Command::cargo_bin("agentics").expect("binary exists");
    update
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unpinned git source"));
}
