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

// ---------------------------------------------------------------------------
// Day cycle: a theme that travels through the day, rather than a modifier.
// ---------------------------------------------------------------------------

/// Anchor palettes with the hour each is at full strength. Circular: the list
/// wraps from the last back to the first, so the small hours flow into dawn.
///
/// Ordered by hour. `theme_at` relies on that and a test enforces it.
pub struct DayAnchor {
  pub hour: f32,
  pub label: &'static str,
  raw: &'static str,
}

pub const DAY_CYCLE: &[DayAnchor] = &[
  DayAnchor {
    hour: 2.0,
    label: "small hours",
    raw: include_str!("../../themes/timeofday/latenight.yml"),
  },
  DayAnchor {
    hour: 5.0,
    label: "dawn",
    raw: include_str!("../../themes/timeofday/dawn.yml"),
  },
  DayAnchor {
    hour: 8.0,
    label: "morning",
    raw: include_str!("../../themes/timeofday/morning.yml"),
  },
  DayAnchor {
    hour: 12.0,
    label: "midday",
    raw: include_str!("../../themes/timeofday/midday.yml"),
  },
  DayAnchor {
    hour: 16.0,
    label: "afternoon",
    raw: include_str!("../../themes/timeofday/afternoon.yml"),
  },
  DayAnchor {
    hour: 19.0,
    label: "dusk",
    raw: include_str!("../../themes/timeofday/dusk.yml"),
  },
  DayAnchor {
    hour: 22.0,
    label: "night",
    raw: include_str!("../../themes/timeofday/night.yml"),
  },
];

impl DayAnchor {
  pub fn theme(&self) -> Theme {
    let wrapper: super::presets::ThemeWrapper =
      serde_yaml::from_str(self.raw).expect("bundled day palette must parse");
    Theme::from(&wrapper.theme)
  }
}

/// The two anchors surrounding `hour`, and how far between them it sits.
///
/// Wraps across midnight, so 23:00 blends night into the small hours rather
/// than falling off the end of the list.
fn surrounding(hour: f32) -> (&'static DayAnchor, &'static DayAnchor, f32) {
  let h = hour.rem_euclid(24.0);
  let last = DAY_CYCLE.len() - 1;

  for i in 0..DAY_CYCLE.len() {
    let a = &DAY_CYCLE[i];
    let b = &DAY_CYCLE[(i + 1) % DAY_CYCLE.len()];
    // Span from a to b, wrapping past midnight for the final pair.
    let span = if b.hour > a.hour {
      b.hour - a.hour
    } else {
      b.hour + 24.0 - a.hour
    };
    let into = if h >= a.hour {
      h - a.hour
    } else {
      h + 24.0 - a.hour
    };
    if into < span {
      return (a, b, into / span);
    }
  }
  // Unreachable while the spans cover 24h, but fall back rather than panic.
  (&DAY_CYCLE[last], &DAY_CYCLE[0], 0.0)
}

/// Theme for an hour of day, interpolated between the surrounding anchors so
/// the palette drifts continuously rather than stepping between phases.
pub fn theme_at(hour: f32) -> Theme {
  let (a, b, t) = surrounding(hour);
  a.theme().blend(b.theme(), t)
}

/// Label of the phase the clock is closest to, for display.
pub fn label_at(hour: f32) -> &'static str {
  let (a, b, t) = surrounding(hour);
  if t < 0.5 {
    a.label
  } else {
    b.label
  }
}

pub fn theme_now() -> Theme {
  theme_at(hour_now())
}

pub fn label_now() -> &'static str {
  label_at(hour_now())
}

fn hour_now() -> f32 {
  let now = Local::now();
  now.hour() as f32 + now.minute() as f32 / 60.0
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn day_anchors_are_sorted_and_within_a_day() {
    let mut prev = -1.0;
    for a in DAY_CYCLE {
      assert!(a.hour > prev, "anchors must ascend: {}", a.hour);
      assert!((0.0..24.0).contains(&a.hour), "{} out of range", a.hour);
      prev = a.hour;
    }
  }

  #[test]
  fn every_day_palette_parses() {
    for a in DAY_CYCLE {
      let _ = a.theme();
    }
  }

  /// Every hour must land inside a span, including the wrap past midnight.
  /// A gap would silently fall through to the unreachable branch.
  #[test]
  fn every_hour_resolves_to_a_span() {
    let mut h = 0.0;
    while h < 24.0 {
      let (a, b, t) = surrounding(h);
      assert!(
        (0.0..1.0).contains(&t),
        "t out of range at {h}: {t} between {} and {}",
        a.label,
        b.label
      );
      h += 0.1;
    }
  }

  /// At an anchor the palette must be exactly that anchor's, not a blend.
  #[test]
  fn anchors_render_their_own_palette_exactly() {
    for a in DAY_CYCLE {
      assert_eq!(
        theme_at(a.hour),
        a.theme(),
        "{} must be exact at its own hour",
        a.label
      );
    }
  }

  /// The wrap is the easy thing to get wrong: 23:00 sits between the night
  /// and small-hours anchors, not off the end of the list.
  #[test]
  fn the_span_across_midnight_blends_night_into_the_small_hours() {
    let (a, b, t) = surrounding(23.0);
    assert_eq!(a.label, "night");
    assert_eq!(b.label, "small hours");
    assert!(t > 0.0 && t < 1.0, "part-way through the span");

    // And just after midnight, still inside the same span.
    let (a2, b2, t2) = surrounding(0.5);
    assert_eq!(a2.label, "night");
    assert_eq!(b2.label, "small hours");
    assert!(t2 > t, "0:30 is further along than 23:00");
  }

  #[test]
  fn day_theme_drifts_rather_than_stepping() {
    // Two nearby times must differ only slightly; two distant ones a lot.
    let close = (theme_at(12.0), theme_at(12.5));
    let far = (theme_at(12.0), theme_at(19.0));
    assert_ne!(far.0, far.1, "midday and dusk must differ");
    assert_ne!(close.0, close.1, "the palette must actually drift");
  }

  #[test]
  fn labels_pick_the_nearer_anchor() {
    assert_eq!(label_at(12.0), "midday");
    assert_eq!(label_at(19.0), "dusk");
    assert_eq!(label_at(2.0), "small hours");
  }

  #[test]
  fn hours_outside_the_day_wrap_for_the_cycle_too() {
    assert_eq!(theme_at(24.0), theme_at(0.0));
    assert_eq!(theme_at(-1.0), theme_at(23.0));
  }

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
