pub mod client;
pub mod daylight;
pub mod keys;
pub mod presets;
pub mod theme;
pub mod user;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

pub const APP_NAME: &str = "spotuify";

pub fn project_dirs() -> Result<ProjectDirs> {
  ProjectDirs::from("io", "", APP_NAME).context("could not determine config directory")
}

pub fn config_dir() -> Result<PathBuf> {
  let dir = project_dirs()?.config_dir().to_path_buf();
  std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
  Ok(dir)
}

pub fn client_config_path() -> Result<PathBuf> {
  Ok(config_dir()?.join("client.yml"))
}

pub fn user_config_path() -> Result<PathBuf> {
  Ok(config_dir()?.join("config.yml"))
}

pub fn token_cache_path() -> Result<PathBuf> {
  Ok(config_dir()?.join(".token_cache.json"))
}

/// Rendered playlist covers, one small file per image. Lives under the OS
/// cache dir rather than the config dir: it is fully derived data and safe to
/// delete at any time.
pub fn cover_cache_dir() -> Result<PathBuf> {
  let dir = project_dirs()?.cache_dir().join("covers");
  std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
  Ok(dir)
}

/// Persists the theme chosen via the in-app picker. Contents: one line, the
/// preset name (e.g. "Nord"). Takes precedence over `config.yml`'s theme block
/// on startup. Delete this file to revert to your `config.yml` theme.
pub fn selected_theme_path() -> Result<PathBuf> {
  Ok(config_dir()?.join(".selected_theme"))
}
