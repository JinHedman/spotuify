use crate::app::AppState;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Gauge, Paragraph},
  Frame,
};
use rspotify::model::{CurrentlyPlayingType, PlayableItem};
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
    let text = if state.is_loading {
      "Loading…"
    } else {
      "Nothing playing. Start Spotify on any device, then press r."
    };
    frame.render_widget(Paragraph::new(text), inner);
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
  let gauge = Gauge::default()
    .gauge_style(Style::default().fg(theme.progress))
    .ratio(ratio)
    .label(label);
  frame.render_widget(gauge, rows[1]);
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
