use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf, process::Command};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub username_ref: String,
    pub password_ref: String,
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

pub fn setup(username_ref: String, password_ref: String) -> Result<()> {
    if !valid_reference(&username_ref) || !valid_reference(&password_ref) {
        return Err(Error::new(
            2,
            "Use op://vault/item/field references, not secret values",
        ));
    }
    let config = Config {
        username_ref,
        password_ref,
    };
    let data = toml::to_string(&config).map_err(|_| Error::new(1, "Cannot serialize config"))?;
    let path = path()?;
    fs::create_dir_all(path.parent().unwrap())
        .map_err(|_| Error::new(1, "Cannot create ~/.mlc"))?;
    fs::write(path, data).map_err(|_| Error::new(1, "Cannot save configuration"))
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
    if !valid_reference(&config.username_ref) || !valid_reference(&config.password_ref) {
        return Err(Error::new(
            2,
            "Configuration must contain 1Password references only",
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
    Ok(Credentials {
        username: resolve(&config.username_ref)?,
        password: resolve(&config.password_ref)?,
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
