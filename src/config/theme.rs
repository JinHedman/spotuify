use ratatui::style::Color;
use serde::{Deserialize, Deserializer};

/// Wrapper that knows how to deserialize any ratatui-compatible color string.
#[derive(Debug, Clone, Copy)]
pub struct ColorCfg(pub Color);

impl ColorCfg {
  pub fn color(self) -> Color {
    self.0
  }
}

impl<'de> Deserialize<'de> for ColorCfg {
  fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
    use serde::de::Error;
    let s: String = String::deserialize(d)?;
    parse_color(&s).map(ColorCfg).map_err(D::Error::custom)
  }
}

fn parse_color(s: &str) -> Result<Color, String> {
  let trimmed = s.trim();
  if let Some(hex) = trimmed.strip_prefix('#') {
    return parse_hex(hex);
  }
  Ok(match trimmed.to_ascii_lowercase().as_str() {
    "reset" => Color::Reset,
    "black" => Color::Black,
    "red" => Color::Red,
    "green" => Color::Green,
    "yellow" => Color::Yellow,
    "blue" => Color::Blue,
    "magenta" => Color::Magenta,
    "cyan" => Color::Cyan,
    "gray" | "grey" => Color::Gray,
    "darkgray" | "darkgrey" => Color::DarkGray,
    "lightred" => Color::LightRed,
    "lightgreen" => Color::LightGreen,
    "lightyellow" => Color::LightYellow,
    "lightblue" => Color::LightBlue,
    "lightmagenta" => Color::LightMagenta,
    "lightcyan" => Color::LightCyan,
    "white" => Color::White,
    _ => return Err(format!("unknown color: {s:?}")),
  })
}

fn parse_hex(hex: &str) -> Result<Color, String> {
  if hex.len() != 6 {
    return Err(format!("hex color must be 6 chars: {hex}"));
  }
  let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
  let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
  let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
  Ok(Color::Rgb(r, g, b))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeCfg {
  pub active: ColorCfg,
  pub inactive: ColorCfg,
  pub selected_bg: ColorCfg,
  pub text: ColorCfg,
  pub hint: ColorCfg,
  pub error: ColorCfg,
  pub progress: ColorCfg,
  pub playing_icon: ColorCfg,
}

impl Default for ThemeCfg {
  fn default() -> Self {
    Self {
      active: ColorCfg(Color::Green),
      inactive: ColorCfg(Color::DarkGray),
      selected_bg: ColorCfg(Color::DarkGray),
      text: ColorCfg(Color::Reset),
      hint: ColorCfg(Color::DarkGray),
      error: ColorCfg(Color::Red),
      progress: ColorCfg(Color::Green),
      playing_icon: ColorCfg(Color::Green),
    }
  }
}

/// Resolved theme (plain `Color` values) used by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
  pub active: Color,
  pub inactive: Color,
  pub selected_bg: Color,
  // Accepted in config.yml for forward compat but not yet read by the UI.
  #[allow(dead_code)]
  pub text: Color,
  pub hint: Color,
  pub error: Color,
  pub progress: Color,
  pub playing_icon: Color,
}

impl From<&ThemeCfg> for Theme {
  fn from(cfg: &ThemeCfg) -> Self {
    Self {
      active: cfg.active.color(),
      inactive: cfg.inactive.color(),
      selected_bg: cfg.selected_bg.color(),
      text: cfg.text.color(),
      hint: cfg.hint.color(),
      error: cfg.error.color(),
      progress: cfg.progress.color(),
      playing_icon: cfg.playing_icon.color(),
    }
  }
}

/// Blend two colours, `t` running 0.0 (all `from`) to 1.0 (all `to`).
///
/// Only true RGB blends. A named or indexed colour has no RGB value of its
/// own — it is whatever the user's terminal palette says it is — so mixing one
/// numerically would mean substituting our guess for their configured colour.
/// Those snap at the midpoint instead. Themes written in hex, which is every
/// bundled preset's accent, blend properly.
pub fn blend(from: Color, to: Color, t: f32) -> Color {
  // Clamp here, not only in `Theme::blend`. `mix` clamps each channel
  // independently, so an out-of-range t saturates every channel separately
  // and yields white rather than the target colour.
  let t = t.clamp(0.0, 1.0);
  match (from, to) {
    (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
      Color::Rgb(mix(r1, r2, t), mix(g1, g2, t), mix(b1, b2, t))
    }
    _ if t < 0.5 => from,
    _ => to,
  }
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
  let a = f32::from(a);
  let b = f32::from(b);
  (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

impl Theme {
  /// Field-by-field blend toward `to`.
  pub fn blend(self, to: Self, t: f32) -> Self {
    let t = t.clamp(0.0, 1.0);
    Self {
      active: blend(self.active, to.active, t),
      inactive: blend(self.inactive, to.inactive, t),
      selected_bg: blend(self.selected_bg, to.selected_bg, t),
      text: blend(self.text, to.text, t),
      hint: blend(self.hint, to.hint, t),
      error: blend(self.error, to.error, t),
      progress: blend(self.progress, to.progress, t),
      playing_icon: blend(self.playing_icon, to.playing_icon, t),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const A: Color = Color::Rgb(0, 0, 0);
  const B: Color = Color::Rgb(255, 128, 64);

  #[test]
  fn blend_hits_both_endpoints_exactly() {
    assert_eq!(blend(A, B, 0.0), A, "t=0 is untouched");
    assert_eq!(blend(A, B, 1.0), B, "t=1 lands exactly on the target");
  }

  #[test]
  fn blend_is_monotonic_between_endpoints() {
    let mut last = 0u8;
    for i in 0..=10 {
      let Color::Rgb(r, _, _) = blend(A, B, i as f32 / 10.0) else {
        panic!("rgb pair must blend to rgb");
      };
      assert!(r >= last, "red channel must not go backwards");
      last = r;
    }
    assert_eq!(last, 255);
  }

  /// The whole point of the snap branch: a named colour is the terminal's to
  /// define, so it must survive a blend rather than being replaced by our
  /// numeric guess at it.
  #[test]
  fn named_colours_snap_and_are_never_rewritten_as_rgb() {
    let named = Color::DarkGray;
    assert_eq!(blend(named, B, 0.0), named);
    assert_eq!(blend(named, B, 0.49), named, "holds until the midpoint");
    assert_eq!(blend(named, B, 0.5), B, "then switches outright");
    assert_eq!(blend(B, named, 0.9), named);
    // Never an interpolated value.
    for i in 0..=10 {
      let out = blend(named, B, i as f32 / 10.0);
      assert!(out == named || out == B, "got an invented colour: {out:?}");
    }
  }

  #[test]
  fn out_of_range_t_is_clamped() {
    assert_eq!(blend(A, B, -5.0), A);
    assert_eq!(blend(A, B, 5.0), B);
  }

  #[test]
  fn theme_blend_moves_every_field() {
    let from = Theme::from(&ThemeCfg::default());
    let to = Theme { active: B, ..from };
    let mid = from.blend(to, 1.0);
    assert_eq!(mid.active, B, "changed field reaches the target");
    assert_eq!(mid.error, from.error, "untouched fields stay put");
  }
}
