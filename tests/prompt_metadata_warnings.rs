use assert_cmd::Command;
use predicates::prelude::*;

fn setup_prompt_with_bad_frontmatter(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("prompts")).unwrap();
    std::fs::write(
        root.join("prompts/review.md"),
        "---\ntitle: [bad\n---\nReview this.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: prompt\n    name: review\n    source: ./prompts/review.md\n    harnesses: [claude]\n",
    )
    .unwrap();
}

#[test]
fn doctor_warns_for_invalid_prompt_frontmatter() {
    let temp = tempfile::tempdir().unwrap();
    setup_prompt_with_bad_frontmatter(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: prompt `review` has invalid YAML frontmatter",
        ));
}

#[test]
fn doctor_json_includes_invalid_prompt_frontmatter_warning() {
    let temp = tempfile::tempdir().unwrap();
    setup_prompt_with_bad_frontmatter(temp.path());

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "prompt `review` has invalid YAML frontmatter",
        ));
}
