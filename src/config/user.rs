use super::keys::KeyBindings;
use super::theme::{Theme, ThemeCfg};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Behavior {
  pub poll_interval_ms: u64,
  pub tick_rate_ms: u64,
  pub volume_step: u8,
  pub seek_step_ms: i64,
}

impl Default for Behavior {
  fn default() -> Self {
    Self {
      poll_interval_ms: 3000,
      tick_rate_ms: 200,
      volume_step: 10,
      seek_step_ms: 5000,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UserConfigFile {
  pub theme: ThemeCfg,
  pub behavior: Behavior,
  pub keybindings: KeyBindings,
}

#[derive(Debug, Clone)]
pub struct UserConfig {
  pub theme: Theme,
  pub behavior: Behavior,
  pub keys: KeyBindings,
}

impl UserConfig {
  pub fn load_or_default(path: &Path) -> Result<Self> {
    let file = if path.exists() {
      let raw =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
      serde_yaml::from_str::<UserConfigFile>(&raw)
        .with_context(|| format!("parsing {}", path.display()))?
    } else {
      UserConfigFile::default()
    };
    Ok(Self {
      theme: Theme::from(&file.theme),
      behavior: file.behavior,
      keys: file.keybindings,
    })
  }
}
