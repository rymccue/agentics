use assert_cmd::Command;
use predicates::prelude::*;

fn git(args: &[&str], cwd: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn setup_git_skill_repo() -> (tempfile::TempDir, String) {
    let repo = tempfile::tempdir().unwrap();
    git(&["init"], repo.path());
    git(&["config", "user.email", "test@example.com"], repo.path());
    git(&["config", "user.name", "Test User"], repo.path());
    let skill = repo.path().join("skills/review");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), "# Review\n").unwrap();
    git(&["add", "."], repo.path());
    git(&["commit", "-m", "add skill"], repo.path());
    let commit = git(&["rev-parse", "HEAD"], repo.path());
    (repo, commit)
}

#[test]
fn sync_dry_run_warns_for_untrusted_git_source() {
    let (_repo_guard, commit) = setup_git_skill_repo();
    let repo_path = _repo_guard.path().display().to_string();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(
        work.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  trustedSources:\n    - github.com/myorg/*\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{repo_path}#{commit}//skills/review\n    harnesses: [claude]\n"
        ),
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(work.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(work.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("untrusted git source"));
}

#[test]
fn non_interactive_sync_blocks_untrusted_git_source_without_yes() {
    let (_repo_guard, commit) = setup_git_skill_repo();
    let repo_path = _repo_guard.path().display().to_string();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(
        work.path().join("agentics.yaml"),
        format!(
            "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  trustedSources:\n    - github.com/myorg/*\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:{repo_path}#{commit}//skills/review\n    harnesses: [claude]\n"
        ),
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(work.path())
        .arg("update")
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(work.path())
        .args(["sync", "--non-interactive"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("policy blocked"));
}
