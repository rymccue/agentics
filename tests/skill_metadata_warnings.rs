use assert_cmd::Command;
use predicates::prelude::*;

fn setup_skill_without_frontmatter(root: &std::path::Path) {
    let skill_dir = root.join("skills/review");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "# Review\n").unwrap();
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: ./skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();
}

#[test]
fn doctor_warns_when_skill_metadata_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    setup_skill_without_frontmatter(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: skill `review` SKILL.md is missing YAML frontmatter",
        ));
}

#[test]
fn doctor_json_includes_skill_metadata_warnings() {
    let temp = tempfile::tempdir().unwrap();
    setup_skill_without_frontmatter(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"warnings\""))
        .stdout(predicate::str::contains(
            "SKILL.md is missing YAML frontmatter",
        ));
}
