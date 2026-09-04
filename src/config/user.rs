use super::keys::KeyBindings;
use super::theme::{Theme, ThemeCfg};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Behavior {
  pub poll_interval_ms: u64,
  pub tick_rate_ms: u64,
  pub volume_step: u8,
  pub seek_step_ms: i64,
  /// Strength of the after-dark warm/dim shift, 0.0 (off) to 1.0 (full).
  ///
  /// Applied on top of whichever theme source is active, so it composes with
  /// decade mode rather than replacing it. Off by default: dimming the UI on
  /// a schedule is a preference, not something to spring on people.
  pub time_of_day_shift: f32,
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
      time_of_day_shift: 0.0,
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

/// A config that loaded, plus whatever went wrong doing it.
///
/// The problem is carried rather than returned as an error because a typo in
/// an optional settings file should not stop the app from starting. The
/// message is surfaced in the UI instead, so it is neither fatal nor silent.
pub struct LoadedConfig {
  pub config: UserConfig,
  pub problem: Option<String>,
}

impl UserConfig {
  /// Load the config, falling back to defaults on any problem.
  ///
  /// Never fails. A missing file is normal; an unreadable or malformed one
  /// gets reported through `LoadedConfig::problem` and the built-in defaults
  /// are used. Refusing to start over one bad line in a file where every
  /// field is optional would be the wrong trade — especially since the file
  /// is hand-edited and the app is the only way to see the result.
  pub fn load(path: &Path) -> LoadedConfig {
    let (file, problem) = Self::read(path);
    LoadedConfig {
      config: Self {
        theme: Theme::from(&file.theme),
        behavior: file.behavior,
        keys: file.keybindings,
      },
      problem,
    }
  }

  fn read(path: &Path) -> (UserConfigFile, Option<String>) {
    if !path.exists() {
      return (UserConfigFile::default(), None);
    }
    let raw = match std::fs::read_to_string(path) {
      Ok(raw) => raw,
      Err(err) => {
        return (
          UserConfigFile::default(),
          Some(format!(
            "could not read {} ({err}) — using defaults",
            path.display()
          )),
        );
      }
    };
    match serde_yaml::from_str::<UserConfigFile>(&raw) {
      Ok(file) => (file, None),
      Err(err) => (
        UserConfigFile::default(),
        // serde_yaml reports line and column, which is the whole point of
        // surfacing this rather than a generic "config invalid".
        Some(format!(
          "{} is invalid ({err}) — using defaults",
          path.display()
        )),
      ),
    }
  }

  /// Test-only shorthand: load and discard any problem.
  ///
  /// Production goes through `load` so the problem is surfaced rather than
  /// dropped; keeping this out of the non-test build stops that being
  /// bypassed by accident.
  #[cfg(test)]
  pub fn load_or_default(path: &Path) -> anyhow::Result<Self> {
    Ok(Self::load(path).config)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  fn write_temp(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("spotuify-config-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(body.as_bytes()).unwrap();
    path
  }

  #[test]
  fn a_missing_file_is_not_a_problem() {
    let loaded = UserConfig::load(std::path::Path::new("/nonexistent/spotuify.yml"));
    assert!(loaded.problem.is_none(), "absent config is the normal case");
    assert_eq!(loaded.config.behavior.volume_step, 10, "defaults applied");
  }

  #[test]
  fn a_valid_file_is_read() {
    let path = write_temp("valid.yml", "behavior:\n  volume_step: 3\n");
    let loaded = UserConfig::load(&path);
    assert!(loaded.problem.is_none());
    assert_eq!(loaded.config.behavior.volume_step, 3);
  }

  /// The point of the change: a typo must not stop the app from starting.
  #[test]
  fn malformed_yaml_falls_back_instead_of_failing() {
    let path = write_temp(
      "broken.yml",
      "behavior:\n  volume_step: [this is not a number\n",
    );
    let loaded = UserConfig::load(&path);
    let problem = loaded.problem.expect("must report the problem");
    assert!(
      problem.contains("broken.yml"),
      "message must name the file: {problem}"
    );
    assert!(
      problem.contains("using defaults"),
      "message must say what happened instead: {problem}"
    );
    assert_eq!(
      loaded.config.behavior.volume_step, 10,
      "defaults must still be usable"
    );
  }

  /// An unknown colour name is a parse failure too, and the message should
  /// carry serde's position rather than a generic complaint.
  #[test]
  fn an_invalid_value_reports_something_actionable() {
    let path = write_temp("badcolour.yml", "theme:\n  active: notacolour\n");
    let loaded = UserConfig::load(&path);
    let problem = loaded.problem.expect("must report the problem");
    assert!(
      problem.contains("notacolour"),
      "message should name the offending value: {problem}"
    );
  }
}
