use super::theme::{Theme, ThemeCfg};
use serde::Deserialize;

#[derive(Deserialize)]
struct Wrapper {
  theme: ThemeCfg,
}

pub struct Preset {
  pub name: &'static str,
  raw: &'static str,
}

/// All preset themes bundled into the binary. Source of truth is `themes/*.yml`
/// at the repo root — these files are `include_str!`-ed at compile time so the
/// preset list stays in sync with the shipped example files.
pub const PRESETS: &[Preset] = &[
  Preset {
    name: "Spotify Green",
    raw: include_str!("../../themes/spotify-green.yml"),
  },
  Preset {
    name: "Gruvbox Dark",
    raw: include_str!("../../themes/gruvbox-dark.yml"),
  },
  Preset {
    name: "Solarized Dark",
    raw: include_str!("../../themes/solarized-dark.yml"),
  },
  Preset {
    name: "Nord",
    raw: include_str!("../../themes/nord.yml"),
  },
  Preset {
    name: "Monokai",
    raw: include_str!("../../themes/monokai.yml"),
  },
];

impl Preset {
  /// Parse the embedded YAML into a resolved Theme. Panics on malformed
  /// preset — the YAML is shipped in-tree so this can only fail if a developer
  /// broke a preset file, which the release build should fail on anyway.
  pub fn theme(&self) -> Theme {
    let wrapper: Wrapper = serde_yaml::from_str(self.raw).expect("bundled preset theme must parse");
    Theme::from(&wrapper.theme)
  }
}

pub fn find_by_name(name: &str) -> Option<&'static Preset> {
  PRESETS.iter().find(|p| p.name == name)
}
