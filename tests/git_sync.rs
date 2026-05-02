use assert_cmd::Command;
use predicates::prelude::*;

fn init_repo(repo: &std::path::Path) -> String {
    std::fs::create_dir_all(repo.join("skills/review")).expect("create skill");
    std::fs::write(repo.join("skills/review/SKILL.md"), "# Review\n").expect("write skill");
    Command::new("git")
        .current_dir(repo)
        .args(["init"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["config", "user.email", "test@example.com"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["config", "user.name", "Test User"])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .assert()
        .success();
    Command::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "initial"])
        .assert()
        .success();
    String::from_utf8(
        Command::new("git")
            .current_dir(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse")
            .stdout,
    )
    .expect("utf8")
    .trim()
    .to_string()
}

#[test]
fn sync_installs_git_skill_after_update() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("resource-repo");
    let commit = init_repo(&repo);
    std::fs::write(
        temp.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{}#{}//skills/review\n    harnesses: [claude]\n",
            repo.display(), commit
        ),
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("sync")
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(temp.path().join(".claude/skills/review/SKILL.md"))
            .expect("installed skill"),
        "# Review\n"
    );
}

#[test]
fn git_sync_dry_run_displays_manifest_source_not_cache_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("resource-repo");
    let commit = init_repo(&repo);
    let source = format!("git:{}#{}//skills/review", repo.display(), commit);
    std::fs::write(
        temp.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: {source}\n    harnesses: [claude]\n",
        ),
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(format!("from {source}"))
                .and(predicate::str::contains(".agentics/cache").not()),
        );
}
