use super::theme::{Theme, ThemeCfg};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ThemeWrapper {
  pub theme: ThemeCfg,
}

/// What selecting a preset does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetKind {
  /// One fixed palette.
  Fixed,
  /// Follows the release decade of whatever is playing, falling back to the
  /// default palette when the year is unknown.
  DecadeAuto,
  /// Follows the release decade like `DecadeAuto`, using the era palettes.
  EraAuto,
  /// Follows the clock, drifting continuously through the day's palettes.
  TimeOfDayAuto,
  /// Not a theme at all — a toggle for the after-dark warm/dim modifier,
  /// which layers on top of whichever theme is selected. Listed here so it is
  /// discoverable in the same place people go to change how the app looks.
  AfterDark,
}

pub struct Preset {
  pub name: &'static str,
  pub kind: PresetKind,
  /// Embedded YAML. Empty for `DecadeAuto`, which has no palette of its own —
  /// it resolves to one of the decade entries at runtime.
  raw: &'static str,
}

/// Palette for a single decade, keyed by the first year of that decade.
pub struct DecadePalette {
  pub decade: u16,
  pub label: &'static str,
  raw: &'static str,
}

/// Decade palettes, oldest first. Source of truth is `themes/decades/*.yml`,
/// `include_str!`-ed at compile time like the other presets.
pub const DECADES: &[DecadePalette] = &[
  DecadePalette {
    decade: 1960,
    label: "1960s",
    raw: include_str!("../../themes/decades/1960s.yml"),
  },
  DecadePalette {
    decade: 1970,
    label: "1970s",
    raw: include_str!("../../themes/decades/1970s.yml"),
  },
  DecadePalette {
    decade: 1980,
    label: "1980s",
    raw: include_str!("../../themes/decades/1980s.yml"),
  },
  DecadePalette {
    decade: 1990,
    label: "1990s",
    raw: include_str!("../../themes/decades/1990s.yml"),
  },
  DecadePalette {
    decade: 2000,
    label: "2000s",
    raw: include_str!("../../themes/decades/2000s.yml"),
  },
  DecadePalette {
    decade: 2010,
    label: "2010s",
    raw: include_str!("../../themes/decades/2010s.yml"),
  },
  DecadePalette {
    decade: 2020,
    label: "2020s",
    raw: include_str!("../../themes/decades/2020s.yml"),
  },
];

/// Era palettes, drawn from documented period colour rather than from how each
/// decade reads on screen now. Starts a decade earlier than `DECADES`.
///
/// Source: https://www.onyxcreative.com/blog/2020/9/popular-color-palettes-by-decade
pub const ERAS: &[DecadePalette] = &[
  DecadePalette {
    decade: 1950,
    label: "1950s",
    raw: include_str!("../../themes/eras/1950s.yml"),
  },
  DecadePalette {
    decade: 1960,
    label: "1960s",
    raw: include_str!("../../themes/eras/1960s.yml"),
  },
  DecadePalette {
    decade: 1970,
    label: "1970s",
    raw: include_str!("../../themes/eras/1970s.yml"),
  },
  DecadePalette {
    decade: 1980,
    label: "1980s",
    raw: include_str!("../../themes/eras/1980s.yml"),
  },
  DecadePalette {
    decade: 1990,
    label: "1990s",
    raw: include_str!("../../themes/eras/1990s.yml"),
  },
  DecadePalette {
    decade: 2000,
    label: "2000s",
    raw: include_str!("../../themes/eras/2000s.yml"),
  },
  DecadePalette {
    decade: 2010,
    label: "2010s",
    raw: include_str!("../../themes/eras/2010s.yml"),
  },
  DecadePalette {
    decade: 2020,
    label: "2020s",
    raw: include_str!("../../themes/eras/2020s.yml"),
  },
];

impl DecadePalette {
  pub fn theme(&self) -> Theme {
    parse(self.raw)
  }
}

/// Map a release year onto a palette from `table`.
///
/// Years outside the table clamp to the nearest end rather than failing: with
/// the decade table a 1954 recording gets the 1960s palette, and anything past
/// the newest entry gets the newest. Returning None would drop the theme back
/// to the fallback, which reads as the feature being broken rather than
/// approximate.
///
/// Shared by both tables so the clamp is written and tested once, and so a
/// table that starts at a different decade — Era begins in the 1950s, Decade
/// in the 1960s — clamps to its own ends rather than a hardcoded one.
pub fn palette_in(table: &'static [DecadePalette], year: u16) -> &'static DecadePalette {
  let decade = year - (year % 10);
  let first = &table[0];
  let last = &table[table.len() - 1];
  if decade <= first.decade {
    return first;
  }
  if decade >= last.decade {
    return last;
  }
  table.iter().find(|d| d.decade == decade).unwrap_or(last)
}

pub fn palette_for_year(year: u16) -> &'static DecadePalette {
  palette_in(DECADES, year)
}

pub fn era_for_year(year: u16) -> &'static DecadePalette {
  palette_in(ERAS, year)
}

/// The app's default palette. Also the fallback for decade mode when a
/// track's release year is unknown — a stable, recognisable colour reads
/// better there than inheriting whichever palette happened to be selected
/// before, which would make the same unknown-year track look different
/// depending on where you had been.
const DEFAULT_RAW: &str = include_str!("../../themes/spotify-green.yml");

pub fn default_theme() -> Theme {
  parse(DEFAULT_RAW)
}

/// All preset themes bundled into the binary. Source of truth is `themes/*.yml`
/// at the repo root — these files are `include_str!`-ed at compile time so the
/// preset list stays in sync with the shipped example files.
pub const PRESETS: &[Preset] = &[
  Preset {
    name: "Spotify Green",
    kind: PresetKind::Fixed,
    raw: DEFAULT_RAW,
  },
  Preset {
    name: "Gruvbox Dark",
    kind: PresetKind::Fixed,
    raw: include_str!("../../themes/gruvbox-dark.yml"),
  },
  Preset {
    name: "Solarized Dark",
    kind: PresetKind::Fixed,
    raw: include_str!("../../themes/solarized-dark.yml"),
  },
  Preset {
    name: "Nord",
    kind: PresetKind::Fixed,
    raw: include_str!("../../themes/nord.yml"),
  },
  Preset {
    name: "Monokai",
    kind: PresetKind::Fixed,
    raw: include_str!("../../themes/monokai.yml"),
  },
  // Listed after the fixed palettes so the existing entries keep their
  // positions and muscle memory still works. The individual decade and era
  // palettes are deliberately not listed: they are the data the auto modes
  // resolve against, not alternatives to choose between, and listing all
  // fifteen buried the five real presets.
  Preset {
    name: "Decade",
    kind: PresetKind::DecadeAuto,
    raw: "",
  },
  Preset {
    name: "Era",
    kind: PresetKind::EraAuto,
    raw: "",
  },
  Preset {
    name: "Time of day",
    kind: PresetKind::TimeOfDayAuto,
    raw: "",
  },
  Preset {
    name: "After dark",
    kind: PresetKind::AfterDark,
    raw: "",
  },
];

fn parse(raw: &str) -> Theme {
  let wrapper: ThemeWrapper = serde_yaml::from_str(raw).expect("bundled preset theme must parse");
  Theme::from(&wrapper.theme)
}

impl Preset {
  /// Parse the embedded YAML into a resolved Theme. Panics on malformed
  /// preset — the YAML is shipped in-tree so this can only fail if a developer
  /// broke a preset file, which the release build should fail on anyway.
  ///
  /// For `DecadeAuto` there is no palette to parse; callers must resolve it
  /// against the playing track instead. Returns None so that is impossible to
  /// get wrong silently.
  pub fn theme(&self) -> Option<Theme> {
    match self.kind {
      PresetKind::Fixed => Some(parse(self.raw)),
      PresetKind::DecadeAuto
      | PresetKind::EraAuto
      | PresetKind::TimeOfDayAuto
      | PresetKind::AfterDark => None,
    }
  }
}

/// Index of the preset with this name, for restoring a persisted choice.
///
/// Returns the index rather than the preset because selecting one goes through
/// `AppState::select_preset`, which needs to know *which* entry it is in order
/// to set the mode — a `&Preset` alone would lose that.
pub fn index_by_name(name: &str) -> Option<usize> {
  PRESETS.iter().position(|p| p.name == name)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Every bundled palette must parse. `theme()` panics on malformed YAML, so
  /// this is the test that turns a broken palette file into a failing build
  /// rather than a crash at runtime.
  #[test]
  fn every_bundled_palette_parses() {
    for p in PRESETS {
      match p.kind {
        PresetKind::Fixed => {
          assert!(p.theme().is_some(), "{} must yield a theme", p.name);
        }
        PresetKind::DecadeAuto
        | PresetKind::EraAuto
        | PresetKind::TimeOfDayAuto
        | PresetKind::AfterDark => {
          assert!(p.theme().is_none(), "{} has no palette of its own", p.name);
        }
      }
    }
    for d in DECADES {
      let _ = d.theme();
    }
  }

  /// The default must stay the same palette as the preset of that name, or
  /// decade mode's fallback would silently diverge from "Spotify Green".
  #[test]
  fn default_theme_is_the_spotify_green_preset() {
    let named = PRESETS
      .iter()
      .find(|p| p.name == "Spotify Green")
      .and_then(|p| p.theme())
      .expect("the Spotify Green preset must exist");
    assert_eq!(default_theme(), named);
  }

  #[test]
  fn decades_are_sorted_and_unique() {
    let mut prev = 0;
    for d in DECADES {
      assert!(d.decade > prev, "decades must ascend: {}", d.decade);
      assert_eq!(d.decade % 10, 0, "{} is not a decade start", d.decade);
      prev = d.decade;
    }
  }

  #[test]
  fn eras_are_sorted_and_unique() {
    let mut prev = 0;
    for e in ERAS {
      assert!(e.decade > prev, "eras must ascend: {}", e.decade);
      assert_eq!(e.decade % 10, 0, "{} is not a decade start", e.decade);
      prev = e.decade;
    }
  }

  #[test]
  fn every_era_palette_parses() {
    for e in ERAS {
      let _ = e.theme();
    }
  }

  /// The two tables start at different decades, so each must clamp to its own
  /// ends. A shared hardcoded bound would give one of them the wrong palette.
  #[test]
  fn each_table_clamps_to_its_own_range() {
    assert_eq!(era_for_year(1900).decade, 1950, "era table reaches 1950s");
    assert_eq!(
      palette_for_year(1900).decade,
      1960,
      "decade table starts at 1960s"
    );
    // A year only the era table covers.
    assert_eq!(era_for_year(1955).decade, 1950);
    assert_eq!(palette_for_year(1955).decade, 1960, "clamped up");
  }

  #[test]
  fn era_years_map_to_their_own_decade() {
    assert_eq!(era_for_year(1958).decade, 1950);
    assert_eq!(era_for_year(1967).decade, 1960);
    assert_eq!(era_for_year(2024).decade, 2020);
  }

  /// Both sets must be selectable and both auto entries present.
  #[test]
  fn both_auto_entries_are_listed() {
    for kind in [PresetKind::DecadeAuto, PresetKind::EraAuto] {
      assert!(
        PRESETS.iter().any(|p| p.kind == kind),
        "{kind:?} must have a picker entry"
      );
    }
    // The individual palettes are intentionally absent from the picker.
    assert!(
      !PRESETS.iter().any(|p| p.name.contains(" · ")),
      "sub-palettes must not be listed as pickable entries"
    );
  }

  #[test]
  fn preset_names_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for p in PRESETS {
      assert!(
        seen.insert(p.name),
        "duplicate preset name {:?} — index_by_name would be ambiguous",
        p.name
      );
    }
  }

  #[test]
  fn years_map_to_their_own_decade() {
    assert_eq!(palette_for_year(1985).decade, 1980);
    assert_eq!(palette_for_year(1980).decade, 1980);
    assert_eq!(palette_for_year(1989).decade, 1980);
    assert_eq!(palette_for_year(1990).decade, 1990);
    assert_eq!(palette_for_year(2024).decade, 2020);
  }

  /// Out-of-table years clamp rather than falling back, so an old recording
  /// still gets a palette instead of looking like the feature failed.
  #[test]
  fn out_of_range_years_clamp_to_the_ends() {
    assert_eq!(palette_for_year(1901).decade, DECADES[0].decade);
    assert_eq!(palette_for_year(1954).decade, DECADES[0].decade);
    assert_eq!(
      palette_for_year(2099).decade,
      DECADES[DECADES.len() - 1].decade
    );
  }

  #[test]
  fn index_by_name_round_trips_every_preset() {
    for p in PRESETS {
      let idx = index_by_name(p.name).expect("preset must be findable by its own name");
      assert_eq!(PRESETS[idx].name, p.name);
      assert_eq!(PRESETS[idx].kind, p.kind);
    }
    assert!(index_by_name("No Such Theme").is_none());
  }
}
