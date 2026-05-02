use assert_cmd::Command;
use predicates::prelude::*;

fn write_skill_with_script(root: &std::path::Path) {
    let skill_dir = root.join("skills/review");
    std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        skill_dir.join("scripts/check.sh"),
        "#!/usr/bin/env bash\necho ok\n",
    )
    .unwrap();
}

fn write_manifest(root: &std::path::Path) {
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();
}

#[test]
fn dry_run_warns_about_executable_content() {
    let temp = tempfile::tempdir().unwrap();
    write_skill_with_script(temp.path());
    write_manifest(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update"])
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("warnings:"))
        .stdout(predicate::str::contains(
            "contains executable content scripts/check.sh",
        ));
}

#[test]
fn dry_run_json_includes_executable_content_warnings() {
    let temp = tempfile::tempdir().unwrap();
    write_skill_with_script(temp.path());
    write_manifest(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["update"])
        .assert()
        .success();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["sync", "--dry-run", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"warnings\""))
        .stdout(predicate::str::contains(
            "contains executable content scripts/check.sh",
        ));
}
