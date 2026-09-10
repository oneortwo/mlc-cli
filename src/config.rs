use crate::error::{Error, Result};
use dialoguer::{Input, Password};
use serde::{Deserialize, Serialize};
use std::{env, fs, io::Write, path::PathBuf};

// Unknown keys are ignored so a config written by an older release still parses.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

pub fn path() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".mlc/config.toml"))
        .ok_or_else(|| Error::new(1, "Cannot locate home directory"))
}

fn read_config() -> Result<Config> {
    let data = fs::read_to_string(path()?).map_err(|_| {
        Error::new(
            2,
            "No credentials configured; run `mlc auth setup` or set MLC_USERNAME and MLC_PASSWORD",
        )
    })?;
    toml::from_str(&data).map_err(|_| Error::new(2, "Invalid ~/.mlc/config.toml"))
}

fn complete(username: Option<&str>, password: Option<&str>) -> bool {
    username.is_some_and(|value| !value.trim().is_empty())
        && password.is_some_and(|value| !value.trim().is_empty())
}

// Both variables set: Some. Neither set: None. Anything else is a misconfiguration.
pub fn from_environment() -> Result<Option<Credentials>> {
    let username = env::var("MLC_USERNAME").ok();
    let password = env::var("MLC_PASSWORD").ok();
    if username.is_none() && password.is_none() {
        return Ok(None);
    }
    if !complete(username.as_deref(), password.as_deref()) {
        return Err(Error::new(
            2,
            "Set both nonempty MLC_USERNAME and MLC_PASSWORD",
        ));
    }
    Ok(Some(Credentials {
        username: username.unwrap_or_default(),
        password: password.unwrap_or_default(),
    }))
}

pub fn prompt() -> Result<Credentials> {
    eprintln!("MLC Public Search API setup");
    let username: String = Input::new()
        .with_prompt("Username")
        .interact_text()
        .map_err(|_| Error::new(2, "Cannot read username"))?;
    let password = Password::new()
        .with_prompt("Password")
        .interact()
        .map_err(|_| Error::new(2, "Cannot read password"))?;
    if !complete(Some(&username), Some(&password)) {
        return Err(Error::new(2, "Username and password must not be empty"));
    }
    Ok(Credentials { username, password })
}

pub fn save(credentials: &Credentials) -> Result<PathBuf> {
    let config = Config {
        username: Some(credentials.username.clone()),
        password: Some(credentials.password.clone()),
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
    file.persist(&path)
        .map_err(|_| Error::new(1, "Cannot save local config"))?;
    Ok(path)
}

pub fn source() -> Result<&'static str> {
    if from_environment()?.is_some() {
        return Ok("environment");
    }
    let config = read_config()?;
    if complete(config.username.as_deref(), config.password.as_deref()) {
        return Ok("config file");
    }
    Err(Error::new(
        2,
        "No credentials configured; run `mlc auth setup` or set MLC_USERNAME and MLC_PASSWORD",
    ))
}

pub fn credentials() -> Result<Credentials> {
    if let Some(credentials) = from_environment()? {
        return Ok(credentials);
    }
    let config = read_config()?;
    match (config.username, config.password) {
        (Some(username), Some(password)) if complete(Some(&username), Some(&password)) => {
            Ok(Credentials { username, password })
        }
        _ => Err(Error::new(
            2,
            "No credentials configured; run `mlc auth setup` or set MLC_USERNAME and MLC_PASSWORD",
        )),
    }
}
