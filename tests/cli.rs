use assert_cmd::Command;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn mlc() -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("mlc"));
    command
        .env_remove("MLC_USERNAME")
        .env_remove("MLC_PASSWORD")
        .env_remove("MLC_API_URL");
    command
}

// A one-shot HTTP server that answers every request with the same body.
fn mock_api(body: &'static str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 8192];
        let count = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..count]).into_owned();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        request
    });
    (address, handle)
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

#[test]
fn setup_without_a_terminal_or_environment_fails_before_any_request() {
    let home = tempfile::tempdir().unwrap();
    let output = mlc()
        .env("HOME", home.path())
        .args(["auth", "setup", "--no-input"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("MLC_USERNAME"));
    assert!(!home.path().join(".mlc/config.toml").exists());
}

#[cfg(unix)]
#[test]
fn setup_verifies_then_saves_private_local_credentials() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let (api, request) = mock_api(r#"{"idToken":"fake-id-token","accessToken":"fake-access"}"#);
    let output = mlc()
        .env("HOME", home.path())
        .env("MLC_API_URL", &api)
        .env("MLC_USERNAME", "fake-user")
        .env("MLC_PASSWORD", "fake-password")
        .args(["auth", "setup", "--no-input"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = request.join().unwrap();
    assert!(request.starts_with("POST /oauth/token "));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("fake-password"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("fake-password"));
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["configured"], true);
    assert_eq!(value["verified"], true);
    let path = home.path().join(".mlc/config.toml");
    let config = std::fs::read_to_string(&path).unwrap();
    assert!(config.contains("fake-password"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let output = mlc()
        .env("HOME", home.path())
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["source"],
        "config file"
    );
}

#[test]
fn setup_does_not_save_credentials_the_api_rejects() {
    let home = tempfile::tempdir().unwrap();
    let (api, request) = mock_api(r#"{"accessToken":"only-an-access-token"}"#);
    let output = mlc()
        .env("HOME", home.path())
        .env("MLC_API_URL", &api)
        .env("MLC_USERNAME", "fake-user")
        .env("MLC_PASSWORD", "fake-password")
        .args(["auth", "setup", "--no-input"])
        .output()
        .unwrap();
    request.join().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!home.path().join(".mlc/config.toml").exists());
}

#[test]
fn config_from_an_older_release_asks_for_setup_instead_of_failing_to_parse() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".mlc")).unwrap();
    std::fs::write(
        home.path().join(".mlc/config.toml"),
        "username_ref = 'op://example/item/username'\npassword_ref = 'op://example/item/password'\n",
    )
    .unwrap();
    let output = mlc()
        .env("HOME", home.path())
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("mlc auth setup"));
}
