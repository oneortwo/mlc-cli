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
fn setup_saves_private_local_credentials_without_op() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let output = mlc()
        .env("HOME", home.path())
        .env("PATH", "")
        .env("MLC_USERNAME", "fake-user")
        .env("MLC_PASSWORD", "fake-password")
        .args(["auth", "setup"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fake-password"));
    let path = home.path().join(".mlc/config.toml");
    let config = std::fs::read_to_string(&path).unwrap();
    assert!(config.contains("fake-password"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let output = mlc()
        .env("HOME", home.path())
        .env("PATH", "")
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["source"],
        "config file"
    );
    mlc()
        .env("HOME", home.path())
        .env("PATH", "")
        .args(["auth", "setup"])
        .assert()
        .success();
}

#[cfg(unix)]
#[test]
fn legacy_references_are_imported_once_and_removed() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".mlc")).unwrap();
    let path = home.path().join(".mlc/config.toml");
    std::fs::write(&path, "username_ref = 'op://example/item/username'\npassword_ref = 'op://example/item/password'\n").unwrap();
    let executable = home.path().join("op");
    std::fs::write(&executable, "#!/bin/sh\nprintf fake-value\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    mlc()
        .env("HOME", home.path())
        .env("PATH", home.path())
        .args(["auth", "setup"])
        .assert()
        .success();
    let data = std::fs::read_to_string(path).unwrap();
    assert!(!data.contains("op://"));
    assert!(!data.contains("_ref"));
    mlc()
        .env("HOME", home.path())
        .env("PATH", "")
        .args(["auth", "setup"])
        .assert()
        .success();
}
