use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{env, fs, io::Write, path::PathBuf, process::Command};

#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_ref: Option<String>,
}

pub struct Credentials {
    pub username: String,
    pub password: String,
}

fn path() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".mlc/config.toml"))
        .ok_or_else(|| Error::new(1, "Cannot locate home directory"))
}

fn read_config() -> Result<Config> {
    let data =
        fs::read_to_string(path()?).map_err(|_| {
            Error::new(2,
        "Set MLC_USERNAME and MLC_PASSWORD, or run mlc auth setup with 1Password references")
        })?;
    toml::from_str(&data).map_err(|_| Error::new(2, "Invalid ~/.mlc/config.toml"))
}

fn valid_reference(reference: &str) -> bool {
    reference.starts_with("op://")
        && reference.split('/').count() >= 5
        && !reference.chars().any(char::is_control)
}

pub fn setup(username_ref: Option<String>, password_ref: Option<String>) -> Result<()> {
    let credentials = match (username_ref, password_ref) {
        (Some(username), Some(password)) => {
            if !valid_reference(&username) || !valid_reference(&password) {
                return Err(Error::new(
                    2,
                    "Use op://vault/item/field references, not secret values",
                ));
            }
            Credentials {
                username: resolve(&username)?,
                password: resolve(&password)?,
            }
        }
        (None, None) => credentials()?,
        _ => return Err(Error::new(2, "Provide both 1Password references")),
    };
    save_credentials(credentials)
}

fn save_credentials(credentials: Credentials) -> Result<()> {
    let config = Config {
        username: Some(credentials.username),
        password: Some(credentials.password),
        ..Config::default()
    };
    let data = toml::to_string(&config).map_err(|_| Error::new(1, "Cannot serialize config"))?;
    let path = path()?;
    let directory = path.parent().unwrap();
    fs::create_dir_all(directory).map_err(|_| Error::new(1, "Cannot create ~/.mlc"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::new(1, "Cannot secure ~/.mlc"))?;
    }
    // NamedTempFile is owner-only on Unix; replacement is atomic.
    let mut file = tempfile::NamedTempFile::new_in(directory)
        .map_err(|_| Error::new(1, "Cannot create local config"))?;
    file.write_all(data.as_bytes())
        .map_err(|_| Error::new(1, "Cannot write local config"))?;
    file.persist(path)
        .map_err(|_| Error::new(1, "Cannot save local config"))?;
    Ok(())
}

pub fn source() -> Result<&'static str> {
    let username = env::var("MLC_USERNAME").ok();
    let password = env::var("MLC_PASSWORD").ok();
    if username.is_some() || password.is_some() {
        if username.as_ref().is_none_or(|v| v.trim().is_empty())
            || password.as_ref().is_none_or(|v| v.trim().is_empty())
        {
            return Err(Error::new(
                2,
                "Set both nonempty MLC_USERNAME and MLC_PASSWORD",
            ));
        }
        return Ok("environment");
    }
    let config = read_config()?;
    if config.username.is_some() || config.password.is_some() {
        if config.username.as_ref().is_none_or(|v| v.trim().is_empty())
            || config.password.as_ref().is_none_or(|v| v.trim().is_empty())
        {
            return Err(Error::new(
                2,
                "Local config needs both nonempty username and password",
            ));
        }
        return Ok("config file");
    }
    if !config.username_ref.as_deref().is_some_and(valid_reference)
        || !config.password_ref.as_deref().is_some_and(valid_reference)
    {
        return Err(Error::new(
            2,
            "Configure local credentials with mlc auth setup",
        ));
    }
    Ok("1password")
}

pub fn credentials() -> Result<Credentials> {
    if source()? == "environment" {
        return Ok(Credentials {
            username: env::var("MLC_USERNAME").unwrap_or_default(),
            password: env::var("MLC_PASSWORD").unwrap_or_default(),
        });
    }
    let config = read_config()?;
    if let (Some(username), Some(password)) = (config.username, config.password) {
        return Ok(Credentials { username, password });
    }
    Ok(Credentials {
        username: resolve(config.username_ref.as_deref().unwrap_or_default())?,
        password: resolve(config.password_ref.as_deref().unwrap_or_default())?,
    })
}

fn resolve(reference: &str) -> Result<String> {
    let output = Command::new("op")
        .args(["read", "--no-newline", reference])
        .output()
        .map_err(|_| Error::new(2, "Cannot start 1Password CLI; install op and sign in"))?;
    if !output.status.success() {
        return Err(Error::new(
            2,
            "1Password read failed; unlock 1Password and check the reference",
        ));
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|_| Error::new(2, "1Password returned invalid text"))?;
    if value.trim().is_empty() {
        return Err(Error::new(2, "1Password credential is empty"));
    }
    Ok(value)
}
