use assert_cmd::Command;

fn mlc() -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("mlc"));
    command
        .env_remove("MLC_USERNAME")
        .env_remove("MLC_PASSWORD");
    command
}

#[test]
fn help_and_completions_need_no_credentials() {
    mlc().arg("--help").assert().success();
    let output = mlc().args(["completions", "zsh"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("#compdef mlc"));
}

#[test]
fn searches_require_criteria() {
    for command in ["works", "recordings"] {
        mlc().args(["search", command]).assert().code(2);
    }
    mlc().args(["work", "batch"]).assert().code(2);
}

#[test]
fn setup_rejects_literal_secrets_without_echoing_them() {
    let output = mlc()
        .args([
            "auth",
            "setup",
            "--username-ref",
            "FAKE_SECRET_USER",
            "--password-ref",
            "FAKE_SECRET_PASSWORD",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("FAKE_SECRET"));
}

#[test]
fn status_does_not_print_environment_secrets() {
    let output = mlc()
        .env("MLC_USERNAME", "FAKE_SECRET_USER")
        .env("MLC_PASSWORD", "FAKE_SECRET_PASSWORD")
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"source":"environment", "verified":false})
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn incomplete_environment_does_not_silently_fall_back() {
    mlc()
        .env("MLC_USERNAME", "fake-user")
        .args(["auth", "status"])
        .assert()
        .code(2);
}

#[cfg(unix)]
#[test]
fn setup_stores_only_references_and_status_does_not_invoke_op() {
    let home = tempfile::tempdir().unwrap();
    mlc()
        .env("HOME", home.path())
        .args([
            "auth",
            "setup",
            "--username-ref",
            "op://example/mlc/username",
            "--password-ref",
            "op://example/mlc/password",
        ])
        .assert()
        .success();
    let config = std::fs::read_to_string(home.path().join(".mlc/config.toml")).unwrap();
    assert!(config.contains("op://example/mlc/username"));
    let output = mlc()
        .env("HOME", home.path())
        .env("PATH", "")
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["source"],
        "1password"
    );
}
