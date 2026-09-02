use crate::app::AppState;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Gauge, Paragraph},
  Frame,
};
use rspotify::model::{CurrentlyPlayingType, PlayableItem, RepeatState};
use serde_json::Value;

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let block = Block::new().borders(Borders::ALL).title(" Now Playing ");
  let inner = block.inner(area);
  frame.render_widget(block, area);

  if let Some(err) = &state.last_error {
    let msg = Paragraph::new(err.as_str()).style(Style::default().fg(theme.error));
    frame.render_widget(msg, inner);
    return;
  }

  let Some(playback) = state.playback.as_ref() else {
    let line = if state.is_loading {
      Line::from(vec![
        Span::styled(spinner_frame(now_ms()), Style::default().fg(theme.active)),
        Span::raw(" Loading…"),
      ])
    } else {
      Line::raw("Nothing playing. Start Spotify on any device, then press r.")
    };
    frame.render_widget(Paragraph::new(line), inner);
    return;
  };

  let rows = Layout::new(
    Direction::Vertical,
    [Constraint::Length(1), Constraint::Length(1)],
  )
  .split(inner);

  let (title, subtitle, duration_ms) = item_display(playback);

  let icon = if playback.is_playing { "▶" } else { "⏸" };
  let vol = playback.device.volume_percent.unwrap_or(0);
  let header = Line::from(vec![
    Span::styled(icon, Style::default().fg(theme.playing_icon)),
    Span::raw("  "),
    Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
    Span::raw("  "),
    Span::styled(subtitle, Style::default().fg(theme.hint)),
    Span::raw("   "),
    Span::styled(format!("vol {vol}%"), Style::default().fg(theme.hint)),
  ]);
  frame.render_widget(Paragraph::new(header), rows[0]);

  let progress_ms = state
    .extrapolated_progress_ms()
    .map(|p| (p.max(0) as u64).min(duration_ms))
    .unwrap_or(0);
  let ratio = if duration_ms > 0 {
    (progress_ms as f64 / duration_ms as f64).clamp(0.0, 1.0)
  } else {
    0.0
  };
  let label = format!("{}  /  {}", format_ms(progress_ms), format_ms(duration_ms));

  // Indicators sit on the timeline row, right-aligned, so the eye finds
  // playback state in one place.
  let bar = Layout::new(
    Direction::Horizontal,
    [Constraint::Min(10), Constraint::Length(MODE_WIDTH)],
  )
  .split(rows[1]);

  let gauge = Gauge::default()
    .gauge_style(Style::default().fg(theme.progress))
    // Without this the bar snaps a whole cell at a time, throwing away the
    // sub-second smoothing that `extrapolated_progress_ms` exists to provide.
    // Unicode eighth-blocks give eight times the horizontal resolution.
    .use_unicode(true)
    .ratio(ratio)
    .label(label);
  frame.render_widget(gauge, bar[0]);
  frame.render_widget(Paragraph::new(mode_line(playback, &theme)), bar[1]);
}

/// Width reserved for the shuffle/repeat indicators: leading space, shuffle
/// glyph, gap, repeat glyph, and the `1` suffix for track-repeat.
const MODE_WIDTH: u16 = 6;

/// Shuffle and repeat state, always rendered — dim when off rather than
/// hidden, so the glyphs never move and their absence can't be mistaken for
/// a rendering gap.
fn mode_line<'a>(
  playback: &rspotify::model::context::CurrentPlaybackContext,
  theme: &crate::config::theme::Theme,
) -> Line<'a> {
  let on = Style::default().fg(theme.active);
  let off = Style::default().fg(theme.hint);

  let shuffle = Span::styled("\u{21c4}", if playback.shuffle_state { on } else { off });
  let (repeat_glyph, repeat_style) = match playback.repeat_state {
    RepeatState::Off => ("\u{21bb} ", off),
    RepeatState::Context => ("\u{21bb} ", on),
    RepeatState::Track => ("\u{21bb}1", on),
  };

  Line::from(vec![
    Span::raw(" "),
    shuffle,
    Span::raw(" "),
    Span::styled(repeat_glyph, repeat_style),
  ])
}

/// Braille spinner, advanced from the wall clock so no frame counter has to be
/// threaded through state. One step per 100ms — twice the redraw tick, which
/// keeps motion visible without looking frantic.
fn spinner_frame(elapsed_ms: u128) -> &'static str {
  const FRAMES: [&str; 10] = [
    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
    "\u{2807}", "\u{280f}",
  ];
  FRAMES[(elapsed_ms / 100) as usize % FRAMES.len()]
}

fn now_ms() -> u128 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis())
    .unwrap_or(0)
}

fn item_display(
  playback: &rspotify::model::context::CurrentPlaybackContext,
) -> (String, String, u64) {
  match &playback.item {
    Some(PlayableItem::Track(t)) => {
      let artists = t
        .artists
        .iter()
        .map(|a| a.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
      (
        t.name.clone(),
        format!("{artists} — {}", t.album.name),
        t.duration.num_milliseconds().max(0) as u64,
      )
    }
    Some(PlayableItem::Episode(e)) => (
      e.name.clone(),
      e.show.name.clone(),
      e.duration.num_milliseconds().max(0) as u64,
    ),
    Some(PlayableItem::Unknown(json)) => unknown_from_json(json),
    None => {
      let label = match playback.currently_playing_type {
        CurrentlyPlayingType::Advertisement => "(ad)",
        CurrentlyPlayingType::Unknown => "(unknown)",
        _ => "(between tracks)",
      };
      (label.to_string(), String::new(), 0)
    }
  }
}

fn unknown_from_json(json: &Value) -> (String, String, u64) {
  let name = json
    .get("name")
    .and_then(|v| v.as_str())
    .unwrap_or("(unrecognized)")
    .to_string();
  let artists = json
    .get("artists")
    .and_then(|v| v.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
    })
    .unwrap_or_default();
  let album = json
    .get("album")
    .and_then(|a| a.get("name"))
    .and_then(|n| n.as_str())
    .unwrap_or("")
    .to_string();
  let duration_ms = json
    .get("duration_ms")
    .and_then(|v| v.as_u64())
    .unwrap_or(0);
  let subtitle = match (artists.is_empty(), album.is_empty()) {
    (false, false) => format!("{artists} — {album}"),
    (false, true) => artists,
    (true, false) => album,
    (true, true) => String::new(),
  };
  (name, subtitle, duration_ms)
}

fn format_ms(ms: u64) -> String {
  let total_secs = ms / 1000;
  let minutes = total_secs / 60;
  let seconds = total_secs % 60;
  format!("{minutes}:{seconds:02}")
}

#[cfg(test)]
mod tests {
  use super::spinner_frame;

  #[test]
  fn spinner_advances_every_100ms_and_cycles() {
    assert_eq!(spinner_frame(0), spinner_frame(99), "same 100ms window");
    assert_ne!(spinner_frame(0), spinner_frame(100), "next window differs");
    assert_eq!(
      spinner_frame(0),
      spinner_frame(1000),
      "wraps after 10 frames"
    );
  }

  /// Driven by the wall clock, so it must not panic or index out of bounds on
  /// arbitrarily large values — this is a real epoch-millis count.
  #[test]
  fn spinner_handles_epoch_scale_values() {
    let mut seen = std::collections::HashSet::new();
    for step in 0..10u128 {
      seen.insert(spinner_frame(1_767_225_600_000 + step * 100));
    }
    assert_eq!(seen.len(), 10, "all ten frames reachable");
    assert!(!spinner_frame(u128::MAX).is_empty());
  }
}
