use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_rejects_high_risk_resource_types_for_mvp() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("agentics.yaml"),
        "apiVersion: agentics.dev/v1alpha1\nkind: AgenticsManifest\nharnesses:\n  pi:\n    enabled: true\ninstall:\n  - type: package\n    name: pi-extension\n    source: git:https://example.com/pkg.git#abc123\n    harnesses: [pi]\n",
    )
    .expect("write manifest");

    Command::cargo_bin("agentics")
        .expect("binary exists")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unsupported high-risk resource type",
        ));
}
