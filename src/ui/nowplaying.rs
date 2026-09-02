//! Marker for the list row holding whatever is currently playing.
//!
//! A three-bar equalizer, animated while playback is running and frozen flat
//! when it is paused. The motion is honest: it reflects that playback is
//! progressing, not any analysis of the audio — spotuify never sees the audio
//! stream. That is the distinction between this and the fake visualisers ruled
//! out in `IDEAS.md`.

/// Characters the glyph occupies. Matches the 3-wide content of the track
/// table's `#` column, so the marker replaces the row number rather than
/// needing a column of its own.
pub const WIDTH: usize = 3;

/// Three bars mid-bounce. Six frames read as movement without implying any
/// particular rhythm.
const FRAMES: [&str; 6] = [
  "\u{2581}\u{2583}\u{2585}",
  "\u{2583}\u{2585}\u{2587}",
  "\u{2585}\u{2587}\u{2585}",
  "\u{2587}\u{2585}\u{2583}",
  "\u{2585}\u{2583}\u{2581}",
  "\u{2583}\u{2581}\u{2583}",
];

/// Paused: bars flat. Distinct from any animation frame, so a paused row can't
/// be mistaken for a slow one.
const PAUSED: &str = "\u{2581}\u{2581}\u{2581}";

/// Matches the redraw tick in `main.rs`. A shorter step would just drop frames,
/// since the UI cannot repaint faster than it redraws — one new frame per
/// redraw is the smoothest this can be.
const STEP_MS: u128 = 200;

/// Whether a list row holds the currently playing item.
///
/// Both arguments are optional — a row can be a local file or an unavailable
/// track, and nothing need be playing. The `playing_uri.is_some()` guard is
/// the point of this function: `None == None` is true, so without it every
/// row lacking a URI would be marked as playing whenever playback was idle.
pub fn is_current(row_uri: Option<&str>, playing_uri: Option<&str>) -> bool {
  playing_uri.is_some() && row_uri == playing_uri
}

pub fn glyph(elapsed_ms: u128, is_playing: bool) -> &'static str {
  if !is_playing {
    return PAUSED;
  }
  FRAMES[(elapsed_ms / STEP_MS) as usize % FRAMES.len()]
}

#[cfg(test)]
mod tests {
  use super::{glyph, FRAMES, PAUSED, WIDTH};

  use super::is_current;

  /// The trap: `None == None` is true, so an unplayable row would be marked
  /// as playing whenever nothing was playing.
  #[test]
  fn rows_without_a_uri_never_match() {
    assert!(!is_current(None, None), "two Nones must not match");
    assert!(!is_current(None, Some("spotify:track:abc")));
    assert!(!is_current(Some("spotify:track:abc"), None));
  }

  #[test]
  fn matches_only_the_same_uri() {
    let playing = Some("spotify:track:abc");
    assert!(is_current(Some("spotify:track:abc"), playing));
    assert!(!is_current(Some("spotify:track:xyz"), playing));
  }

  /// Episodes are matched the same way, so a podcast row marks correctly.
  #[test]
  fn matches_episode_uris_too() {
    let playing = Some("spotify:episode:def");
    assert!(is_current(Some("spotify:episode:def"), playing));
    assert!(!is_current(Some("spotify:track:def"), playing));
  }

  #[test]
  fn paused_is_flat_and_never_an_animation_frame() {
    assert_eq!(glyph(0, false), PAUSED);
    assert_eq!(glyph(12_345_678, false), PAUSED, "regardless of the clock");
    assert!(
      !FRAMES.contains(&PAUSED),
      "paused must be visually distinct from every playing frame"
    );
  }

  #[test]
  fn playing_advances_with_the_redraw_tick_and_cycles() {
    assert_eq!(glyph(0, true), glyph(199, true), "same tick window");
    assert_ne!(glyph(0, true), glyph(200, true), "next window differs");
    assert_eq!(
      glyph(0, true),
      glyph(200 * 6, true),
      "wraps after six frames"
    );
  }

  /// Fed real epoch millis, so it must not panic or index out of bounds.
  #[test]
  fn handles_epoch_scale_values() {
    let mut seen = std::collections::HashSet::new();
    for step in 0..6u128 {
      seen.insert(glyph(1_767_225_600_000 + step * 200, true));
    }
    assert_eq!(seen.len(), 6, "all six frames reachable");
    assert!(!glyph(u128::MAX, true).is_empty());
  }

  /// The glyph sits in a fixed-width column, so every frame must be the same
  /// display width or the column will jitter as it animates.
  #[test]
  fn every_frame_is_exactly_the_column_width() {
    for f in FRAMES.iter().chain(std::iter::once(&PAUSED)) {
      assert_eq!(f.chars().count(), WIDTH, "{f:?} must be {WIDTH} chars wide");
    }
  }
}
