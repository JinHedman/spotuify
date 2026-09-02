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
  /// How long a theme change takes to fade, in milliseconds. 0 snaps.
  ///
  /// Only blends between RGB colours — named colours belong to the terminal
  /// palette and snap at the midpoint instead. See `config::theme::blend`.
  pub theme_transition_ms: u64,
  /// Hide playlists you follow but did not create.
  ///
  /// Spotify's Feb 2026 restriction on `/playlists/{id}/items` means apps
  /// without extended quota get a 403 listing anyone else's playlist, so
  /// followed playlists show an error row instead of tracks. Defaults to
  /// hiding them. Set false to show them again — `Enter` still starts
  /// playback on them via Spotify Connect even though the listing fails.
  pub only_own_playlists: bool,
}

impl Default for Behavior {
  fn default() -> Self {
    Self {
      poll_interval_ms: 3000,
      tick_rate_ms: 200,
      volume_step: 10,
      seek_step_ms: 5000,
      theme_transition_ms: 350,
      only_own_playlists: true,
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
