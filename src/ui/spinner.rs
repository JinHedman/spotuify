//! Shared loading spinner.
//!
//! Advanced from the wall clock rather than a frame counter, so no pane has to
//! thread animation state through `AppState` — every spinner on screen is
//! automatically in phase with every other one.

use crate::config::theme::Theme;
use ratatui::{
  style::Style,
  text::{Line, Span},
};

/// Braille frames. One step per 100ms against the 200ms redraw tick, which
/// keeps the motion visible without looking frantic.
const FRAMES: [&str; 10] = [
  "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
  "\u{2807}", "\u{280f}",
];
const STEP_MS: u128 = 100;

pub fn frame(elapsed_ms: u128) -> &'static str {
  FRAMES[(elapsed_ms / STEP_MS) as usize % FRAMES.len()]
}

pub fn now_ms() -> u128 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis())
    .unwrap_or(0)
}

/// A spinner followed by `label`, for panes waiting on data.
pub fn line<'a>(label: &'a str, theme: &Theme) -> Line<'a> {
  Line::from(vec![
    Span::styled(frame(now_ms()), Style::default().fg(theme.active)),
    Span::raw(" "),
    Span::styled(label, Style::default().fg(theme.hint)),
  ])
}

#[cfg(test)]
mod tests {
  use super::frame;

  #[test]
  fn advances_every_100ms_and_cycles() {
    assert_eq!(frame(0), frame(99), "same 100ms window");
    assert_ne!(frame(0), frame(100), "next window differs");
    assert_eq!(frame(0), frame(1000), "wraps after ten frames");
  }

  /// Driven by real epoch millis, so it must not panic or index out of bounds
  /// on arbitrarily large values.
  #[test]
  fn handles_epoch_scale_values() {
    let mut seen = std::collections::HashSet::new();
    for step in 0..10u128 {
      seen.insert(frame(1_767_225_600_000 + step * 100));
    }
    assert_eq!(seen.len(), 10, "all ten frames reachable");
    assert!(!frame(u128::MAX).is_empty());
  }
}
