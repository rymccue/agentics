use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn update_rejects_git_subpath_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    std::fs::create_dir_all(repo.join("skills/review")).expect("create skill dir");
    std::fs::write(repo.join("skills/review/SKILL.md"), "# Review\n").expect("write skill");
    Command::new("git")
        .current_dir(&repo)
        .arg("init")
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
    .expect("utf8")
    .trim()
    .to_string();

    std::fs::write(
        temp.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{}#{}//../repo/skills/review\n    harnesses: [claude]\n",
            repo.display(), commit
        ),
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "git source subpath escapes checkout",
        ));
}
