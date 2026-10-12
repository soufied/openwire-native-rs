use crate::config::settings::Settings;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub fn config_file() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("OpenWire")
        .join("settings.toml")
}

pub fn default_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home directory"))?;
    let base = dirs::document_dir()
        .filter(|d| d.is_dir())
        .or_else(|| Some(home.join("Documents")).filter(|d| d.is_dir()))
        .unwrap_or(home);
    Ok(base.join("OpenWire"))
}

pub fn data_dir(settings: &Settings) -> Result<PathBuf> {
    match &settings.data_dir {
        Some(dir) => Ok(dir.clone()),
        None => default_data_dir(),
    }
}

pub fn presets_dir(settings: &Settings) -> Result<PathBuf> {
    match &settings.presets_dir {
        Some(dir) => Ok(dir.clone()),
        None => Ok(data_dir(settings)?.join("Voices")),
    }
}

pub fn ensure(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    Ok(())
}
