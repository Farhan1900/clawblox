use std::path::PathBuf;

pub const CLAWBLOX_HOME_ENV: &str = "CLAWBLOX_HOME";

pub fn clawblox_home() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os(CLAWBLOX_HOME_ENV) {
        if !value.is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    home::home_dir()
        .map(|home| home.join(".clawblox"))
        .ok_or_else(|| "failed to determine home directory; set CLAWBLOX_HOME".to_string())
}

pub fn credentials_path() -> Result<PathBuf, String> {
    Ok(clawblox_home()?.join("credentials.toml"))
}

pub fn world_cache_dir() -> Result<PathBuf, String> {
    Ok(clawblox_home()?.join("worlds"))
}

pub fn pid_dir() -> Result<PathBuf, String> {
    Ok(clawblox_home()?.join("run"))
}
