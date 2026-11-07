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
    Ok(config_root()?.join("OpenWire"))
}

fn config_root() -> Result<PathBuf> {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| anyhow!("no config directory"))
}

fn legacy_data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let base = dirs::document_dir()
        .filter(|d| d.is_dir())
        .or_else(|| Some(home.join("Documents")).filter(|d| d.is_dir()))
        .unwrap_or(home);
    Some(base.join("OpenWire"))
}

pub fn migrate_legacy_data(data: &Path) {
    let Some(old) = legacy_data_dir() else {
        return;
    };
    if old == data || data.join("soundboard.toml").exists() || !old.join("soundboard.toml").exists() {
        return;
    }
    let _ = std::fs::copy(old.join("soundboard.toml"), data.join("soundboard.toml"));
    if let Ok(entries) = std::fs::read_dir(old.join("Voices")) {
        let target = data.join("Voices");
        if std::fs::create_dir_all(&target).is_ok() {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = std::fs::copy(&path, target.join(entry.file_name()));
                }
            }
        }
    }
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
