//! Time-of-day warmth applied on top of whatever theme source is active.
//!
//! A modifier rather than a theme of its own, so it composes: decade mode can
//! be running and still get warmed after dark. Deliberately not a source —
//! as one it would have to compete with the others instead of layering.

use super::theme::Theme;
use chrono::{Local, Timelike};
use ratatui::style::Color;

/// Hours bounding the curve. Neutral through the working day, deepest in the
/// small hours, with ramps rather than steps so there is no visible jump as a
/// boundary is crossed.
const DAY_START: f32 = 9.0;
const DAY_END: f32 = 17.0;
const NIGHT_START: f32 = 23.0;
const NIGHT_END: f32 = 5.0;

/// How far each channel is pulled at full warmth. Blue drops most, red not at
/// all — that is what reads as warmth rather than as a colour cast.
const BLUE_PULL: f32 = 0.28;
const GREEN_PULL: f32 = 0.08;
/// Overall dimming at full warmth. Kept modest: the point is to take the edge
/// off, not to make the UI hard to read in a dark room.
const DIM: f32 = 0.18;

/// Warmth for an hour of day, 0.0 (neutral daylight) to 1.0 (deepest night).
///
/// `hour` is fractional, so 17.5 is half past five.
pub fn warmth_at(hour: f32) -> f32 {
  let h = hour.rem_euclid(24.0);
  if (DAY_START..DAY_END).contains(&h) {
    return 0.0;
  }
  if !(NIGHT_END..NIGHT_START).contains(&h) {
    return 1.0;
  }
  if (DAY_END..NIGHT_START).contains(&h) {
    // Evening: ramp up.
    return (h - DAY_END) / (NIGHT_START - DAY_END);
  }
  // Morning: ramp back down.
  1.0 - (h - NIGHT_END) / (DAY_START - NIGHT_END)
}

/// Warmth for the current local time.
pub fn warmth_now() -> f32 {
  let now = Local::now();
  let hour = now.hour() as f32 + now.minute() as f32 / 60.0;
  warmth_at(hour)
}

/// Warm and dim a single colour.
///
/// Named and indexed colours pass through untouched, for the same reason
/// `theme::blend` refuses to interpolate them: their RGB is the terminal's to
/// decide, and we cannot adjust a value we cannot read. A theme mixing hex and
/// named colours therefore warms only partially.
pub fn warm(color: Color, strength: f32) -> Color {
  let w = strength.clamp(0.0, 1.0);
  if w == 0.0 {
    return color;
  }
  let Color::Rgb(r, g, b) = color else {
    return color;
  };
  let dim = 1.0 - DIM * w;
  Color::Rgb(
    scale(r, dim),
    scale(g, dim * (1.0 - GREEN_PULL * w)),
    scale(b, dim * (1.0 - BLUE_PULL * w)),
  )
}

fn scale(v: u8, factor: f32) -> u8 {
  (f32::from(v) * factor).round().clamp(0.0, 255.0) as u8
}

/// Apply warmth across a whole theme.
///
/// `error` is left alone: it is the one colour whose job is to be alarming,
/// and warming it toward the accent range at night would blunt exactly the
/// signal it exists to send.
pub fn warm_theme(theme: Theme, strength: f32) -> Theme {
  Theme {
    active: warm(theme.active, strength),
    inactive: warm(theme.inactive, strength),
    selected_bg: warm(theme.selected_bg, strength),
    text: warm(theme.text, strength),
    hint: warm(theme.hint, strength),
    error: theme.error,
    progress: warm(theme.progress, strength),
    playing_icon: warm(theme.playing_icon, strength),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn midday_is_neutral_and_small_hours_are_full() {
    for h in [9.0, 12.0, 16.9] {
      assert_eq!(warmth_at(h), 0.0, "{h} should be neutral");
    }
    for h in [23.0, 23.5, 0.0, 3.0, 4.9] {
      assert_eq!(warmth_at(h), 1.0, "{h} should be full warmth");
    }
  }

  #[test]
  fn ramps_are_monotonic_and_bounded() {
    // Evening: rising.
    let mut prev = -1.0;
    let mut h = 17.0;
    while h < 23.0 {
      let w = warmth_at(h);
      assert!((0.0..=1.0).contains(&w), "out of range at {h}: {w}");
      assert!(w >= prev, "evening must rise: {h}");
      prev = w;
      h += 0.25;
    }
    // Morning: falling.
    let mut prev = 2.0;
    let mut h = 5.0;
    while h < 9.0 {
      let w = warmth_at(h);
      assert!((0.0..=1.0).contains(&w), "out of range at {h}: {w}");
      assert!(w <= prev, "morning must fall: {h}");
      prev = w;
      h += 0.25;
    }
  }

  /// No discontinuity at a boundary, or the theme would visibly jump.
  #[test]
  fn curve_is_continuous_across_boundaries() {
    for edge in [5.0, 9.0, 17.0, 23.0] {
      let before = warmth_at(edge - 0.01);
      let after = warmth_at(edge + 0.01);
      assert!(
        (before - after).abs() < 0.02,
        "jump at {edge}: {before} -> {after}"
      );
    }
  }

  #[test]
  fn hours_wrap_rather_than_panicking() {
    assert_eq!(warmth_at(24.0), warmth_at(0.0));
    assert_eq!(warmth_at(25.0), warmth_at(1.0));
    assert_eq!(warmth_at(-1.0), warmth_at(23.0));
  }

  #[test]
  fn zero_strength_is_the_identity() {
    let c = Color::Rgb(30, 200, 250);
    assert_eq!(warm(c, 0.0), c);
    let t = sample_theme();
    assert_eq!(warm_theme(t, 0.0), t);
  }

  #[test]
  fn warming_pulls_blue_hardest_and_never_raises_a_channel() {
    let Color::Rgb(r, g, b) = warm(Color::Rgb(200, 200, 200), 1.0) else {
      panic!("rgb in, rgb out");
    };
    assert!(r <= 200 && g <= 200 && b <= 200, "no channel may rise");
    assert!(b < g, "blue must fall further than green");
    assert!(g < r, "green must fall further than red");
  }

  /// Same rule as `theme::blend`: a named colour belongs to the terminal.
  #[test]
  fn named_colours_pass_through_untouched() {
    for c in [
      Color::DarkGray,
      Color::Red,
      Color::Reset,
      Color::Indexed(42),
    ] {
      assert_eq!(warm(c, 1.0), c, "{c:?} must be left alone");
    }
  }

  /// Errors must stay alarming even at 3am.
  #[test]
  fn error_colour_is_never_warmed() {
    let t = sample_theme();
    let warmed = warm_theme(t, 1.0);
    assert_eq!(warmed.error, t.error);
    assert_ne!(warmed.active, t.active, "but the rest does change");
  }

  #[test]
  fn out_of_range_strength_is_clamped() {
    let c = Color::Rgb(200, 200, 200);
    assert_eq!(warm(c, 5.0), warm(c, 1.0));
    assert_eq!(warm(c, -5.0), c);
  }

  fn sample_theme() -> Theme {
    Theme {
      active: Color::Rgb(29, 185, 84),
      inactive: Color::Rgb(80, 80, 80),
      selected_bg: Color::Rgb(40, 40, 40),
      text: Color::Rgb(220, 220, 220),
      hint: Color::Rgb(120, 120, 120),
      error: Color::Rgb(255, 0, 0),
      progress: Color::Rgb(29, 185, 84),
      playing_icon: Color::Rgb(29, 185, 84),
    }
  }
}
