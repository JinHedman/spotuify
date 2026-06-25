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
