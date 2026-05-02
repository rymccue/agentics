use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_warns_for_untrusted_git_source() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  trustedSources:\n    - github.com/myorg/*\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:https://github.com/other/repo.git#v1//skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: untrusted git source `https://github.com/other/repo.git`",
        ));
}

#[test]
fn doctor_does_not_warn_for_trusted_git_source() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  trustedSources:\n    - github.com/myorg/*\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:https://github.com/myorg/repo.git#v1//skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains("untrusted git source").not());
}

#[test]
fn doctor_json_includes_untrusted_source_warning() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\npolicy:\n  trustedSources:\n    - github.com/myorg/*\nharnesses:\n  claude:\n    enabled: true\ninstall:\n  - type: skill\n    name: review\n    source: git:https://github.com/other/repo.git#v1//skills/review\n    harnesses: [claude]\n",
    )
    .unwrap();

    Command::cargo_bin("agentics")
        .unwrap()
        .current_dir(temp.path())
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("untrusted git source"));
}
