use crate::{
    config::Credentials,
    error::{Error, Result},
};
use reqwest::{
    blocking::{Client as HttpClient, Response},
    redirect::Policy,
    Method,
};
use serde_json::{json, Value};
use std::time::Duration;

const BASE_URL: &str = "https://public-api.themlc.com/";

/// Separate, unauthenticated transport: never attach MLC credentials to GitHub.
pub fn release_download(url: &str) -> Result<Vec<u8>> {
    HttpClient::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(15))
        .user_agent(concat!("mlc-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| Error::new(1, "Cannot initialize update client"))?
        .get(url)
        .send()
        .and_then(Response::error_for_status)
        .and_then(Response::bytes)
        .map(|bytes| bytes.to_vec())
        .map_err(|_| Error::new(1, "GitHub update request failed; try again later"))
}

pub struct Client {
    http: HttpClient,
    base: String,
    token: String,
    secrets: Vec<String>,
}

impl Client {
    pub fn login(credentials: Credentials) -> Result<Self> {
        // MLC_API_URL exists for tests and proxies; the default is the public MLC host.
        let base = std::env::var("MLC_API_URL")
            .ok()
            .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
            .map(|url| format!("{}/", url.trim_end_matches('/')))
            .unwrap_or_else(|| BASE_URL.to_string());
        Self::login_at(&base, credentials)
    }

    fn login_at(base: &str, credentials: Credentials) -> Result<Self> {
        let http = HttpClient::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            .user_agent(concat!("mlc-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::new(1, "Cannot initialize HTTP client"))?;
        let response = http
            .post(format!("{base}oauth/token"))
            .json(&json!({"username": credentials.username, "password": credentials.password}))
            .send()
            .map_err(|_| Error::new(1, "MLC authentication connection failed"))?;
        let body = decode(response)?;
        // MLC's data endpoints require the JWT idToken, not the opaque accessToken.
        let token = body
            .get("idToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| Error::new(2, "MLC did not return an ID token; check credentials"))?;
        // Only tokens are scrubbed from responses. Usernames and passwords are never
        // echoed by the API, and scrubbing them would corrupt titles that happen to
        // contain the same text.
        let mut secrets = vec![token.to_string()];
        for key in ["refreshToken", "accessToken"] {
            if let Some(value) = body.get(key).and_then(Value::as_str) {
                secrets.push(value.into());
            }
        }
        Ok(Self {
            http,
            base: base.into(),
            token: token.into(),
            secrets,
        })
    }

    pub fn request(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .map_err(|_| Error::new(1, "MLC request failed (connection or timeout)"))?;
        let mut result = decode(response)?;
        redact(&mut result, &self.secrets);
        Ok(result)
    }

    pub fn work(&self, id: &str) -> Result<Value> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(Error::new(
                1,
                "MLC song codes must contain only letters, digits or hyphens",
            ));
        }
        self.request(Method::GET, &format!("work/id/{id}"), None)
    }
}

fn decode(response: Response) -> Result<Value> {
    let status = response.status();
    if !status.is_success() {
        let (code, message) = match status.as_u16() {
            400 => (
                1,
                "MLC rejected the query; check search criteria and identifier values",
            ),
            401 | 403 => (2, "MLC rejected authentication or access"),
            404 => (3, "MLC resource not found"),
            429 => (4, "MLC rate limit reached; retry later"),
            _ => (1, "MLC request was unsuccessful"),
        };
        return Err(Error::new(code, &format!("{message} (HTTP {status})")));
    }
    response
        .json()
        .map_err(|_| Error::new(1, "MLC returned invalid JSON"))
}

fn redact(value: &mut Value, secrets: &[String]) {
    match value {
        Value::String(text) => {
            for secret in secrets.iter().filter(|s| !s.is_empty()) {
                *text = text.replace(secret, "[REDACTED]");
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact(item, secrets)),
        Value::Object(object) => {
            let original = std::mem::take(object);
            for (key, mut item) in original {
                let mut safe_key = Value::String(key);
                redact(&mut safe_key, secrets);
                redact(&mut item, secrets);
                object.insert(safe_key.as_str().unwrap().into(), item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn server(status: &str, body: &str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/", listener.local_addr().unwrap());
        let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut buffer = [0; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..count]);
                let text = String::from_utf8_lossy(&request);
                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|n| n.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
                assert!(count > 0);
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (address, handle)
    }

    #[test]
    fn release_download_uses_no_authentication() {
        let (address, handle) = server("200 OK", "archive");
        assert_eq!(release_download(&address).unwrap(), b"archive");
        let request = handle.join().unwrap().to_lowercase();
        assert!(request.starts_with("get / "));
        assert!(!request.contains("authorization:"));
        assert!(request.contains("user-agent: mlc-cli/"));
    }

    #[test]
    fn release_download_rejects_http_errors_without_echoing_body() {
        for status in [
            "404 Not Found",
            "429 Too Many Requests",
            "500 Internal Server Error",
        ] {
            let (address, handle) = server(status, "untrusted response");
            let error = release_download(&address).unwrap_err();
            assert_eq!(error.code, 1);
            assert!(!error.message.contains("untrusted response"));
            handle.join().unwrap();
        }
    }

    #[test]
    fn login_uses_documented_body_without_returning_tokens() {
        let (address, handle) = server(
            "200 OK",
            r#"{"accessToken":"fake-access-token","idToken":"fake-id-token"}"#,
        );
        let client = Client::login_at(
            &address,
            Credentials {
                username: "fake-user".into(),
                password: "fake-password".into(),
            },
        )
        .unwrap();
        assert_eq!(client.token, "fake-id-token");
        assert_eq!(client.secrets, vec!["fake-id-token", "fake-access-token"]);
        let request = handle.join().unwrap();
        assert!(request.starts_with("POST /oauth/token "));
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            body,
            json!({"username":"fake-user", "password":"fake-password"})
        );
    }

    #[test]
    fn access_token_alone_is_not_accepted_as_data_authentication() {
        let (address, handle) = server("200 OK", r#"{"accessToken":"fake-access-token"}"#);
        let result = Client::login_at(
            &address,
            Credentials {
                username: "fake-user".into(),
                password: "fake-password".into(),
            },
        );
        let Err(error) = result else {
            panic!("Expected missing ID token error")
        };
        assert_eq!(error.code, 2);
        assert!(!error.to_string().contains("fake-access-token"));
        handle.join().unwrap();
    }

    #[test]
    fn malformed_success_response_does_not_leak_body() {
        let (address, handle) = server("200 OK", "FAKE_SECRET_NOT_JSON");
        let response = HttpClient::new().get(address).send().unwrap();
        let error = decode(response).unwrap_err();
        assert_eq!(error.code, 1);
        assert!(!error.to_string().contains("FAKE_SECRET"));
        handle.join().unwrap();
    }

    #[test]
    fn errors_never_echo_response_secrets() {
        for (status, code) in [
            ("401 Unauthorized", 2),
            ("403 Forbidden", 2),
            ("404 Not Found", 3),
            ("429 Too Many Requests", 4),
            ("500 Server Error", 1),
            ("302 Found", 1),
        ] {
            let (address, handle) = server(status, "SECRET");
            let response = HttpClient::new().get(address).send().unwrap();
            let error = decode(response).unwrap_err();
            assert_eq!(error.code, code);
            assert!(!error.to_string().contains("SECRET"));
            handle.join().unwrap();
        }
    }

    #[test]
    fn request_uses_bearer_and_redacts_echoed_tokens_but_not_titles() {
        let (address, handle) = server("200 OK", r#"[{"title":"Love","fake-token":"fake-token"}]"#);
        let client = Client {
            http: HttpClient::new(),
            base: address,
            token: "fake-token".into(),
            secrets: vec!["fake-token".into()],
        };
        let result = client
            .request(Method::POST, "works", Some(json!([{"mlcsongCode":"123"}])))
            .unwrap();
        assert_eq!(result, json!([{"title":"Love","[REDACTED]":"[REDACTED]"}]));
        let request = handle.join().unwrap();
        assert!(request
            .to_lowercase()
            .contains("authorization: bearer fake-token"));
        assert!(request.contains(r#"[{"mlcsongCode":"123"}]"#));
    }
}
