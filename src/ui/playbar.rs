use crate::app::{AppState, NOWPLAYING_COLS};
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::Color,
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
      crate::ui::spinner::line("Loading…", &theme)
    } else {
      Line::raw("Nothing playing. Start Spotify on any device, then press r.")
    };
    frame.render_widget(Paragraph::new(line), inner);
    return;
  };

  // Cover on the left; everything else takes the full remaining width so the
  // progress bar is as long as the pane allows.
  let cols = Layout::new(
    Direction::Horizontal,
    [Constraint::Length(NOWPLAYING_COLS + 2), Constraint::Min(20)],
  )
  .split(inner);

  draw_thumbnail(frame, cols[0], state, &theme);

  let (title, subtitle, album_line, duration_ms) = item_display(playback);

  let rows = Layout::new(
    Direction::Vertical,
    [
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Length(1),
    ],
  )
  .split(cols[1]);

  // Row 1: everything identifying the track, on one line. Stacking it over
  // three rows was what forced the bar into a narrow column.
  let icon = if playback.is_playing { "▶" } else { "⏸" };
  let mut ident = vec![
    Span::styled(icon, Style::default().fg(theme.playing_icon)),
    Span::raw(" "),
    Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
  ];
  for part in [subtitle, album_line] {
    if !part.is_empty() {
      ident.push(Span::styled(" · ", Style::default().fg(theme.inactive)));
      ident.push(Span::styled(part, Style::default().fg(theme.hint)));
    }
  }
  frame.render_widget(Paragraph::new(Line::from(ident)), rows[0]);

  let progress_ms = state
    .extrapolated_progress_ms()
    .map(|p| (p.max(0) as u64).min(duration_ms))
    .unwrap_or(0);
  let ratio = if duration_ms > 0 {
    (progress_ms as f64 / duration_ms as f64).clamp(0.0, 1.0)
  } else {
    0.0
  };

  // Row 2: time and playback state, centred over the bar beneath them.
  let vol = playback.device.volume_percent.unwrap_or(0);
  let mut status = vec![Span::styled(
    format!("{} / {}", format_ms(progress_ms), format_ms(duration_ms)),
    Style::default().fg(theme.hint),
  )];
  status.push(Span::raw("    "));
  status.extend(mode_line(playback, &theme).spans);
  status.push(Span::raw("    "));
  status.push(Span::styled(
    format!("vol {vol}%"),
    volume_style(state, &theme),
  ));
  frame.render_widget(Paragraph::new(Line::from(status)).centered(), rows[1]);

  // Row 3: the bar, full width of the column.
  let gauge = Gauge::default()
    .gauge_style(Style::default().fg(theme.progress))
    // Sub-cell resolution, so the extrapolated progress reads as motion
    // rather than snapping a whole cell at a time.
    .use_unicode(true)
    .ratio(ratio)
    .label("");
  frame.render_widget(gauge, rows[2]);
}

/// The playbar thumbnail, or nothing when there is no art to show.
///
/// Silent when absent: the playbar is not the place to explain a missing
/// cover, and a placeholder would draw the eye to the one thing that has
/// nothing to say.
fn draw_thumbnail(
  frame: &mut Frame,
  area: Rect,
  state: &AppState,
  theme: &crate::config::theme::Theme,
) {
  let playing_uri = state.playing_uri();
  let art = state
    .now_playing_cover
    .as_ref()
    .filter(|c| Some(&c.id) == playing_uri.as_ref())
    .and_then(|c| c.art.as_ref());

  let Some(art) = art else {
    return;
  };

  let lines: Vec<Line> = (0..art.rows as usize)
    .map(|row| {
      Line::from(
        (0..art.cols as usize)
          .filter_map(|col| art.cells.get(row * art.cols as usize + col))
          .map(|&((tr, tg, tb), (br, bg, bb))| {
            Span::styled(
              "\u{2580}",
              Style::default()
                .fg(Color::Rgb(tr, tg, tb))
                .bg(Color::Rgb(br, bg, bb)),
            )
          })
          .collect::<Vec<_>>(),
      )
    })
    .collect();

  let _ = theme;
  let width = art.cols.min(area.width);
  let target = Rect {
    x: area.x,
    y: area.y,
    width,
    height: art.rows.min(area.height),
  };
  frame.render_widget(Paragraph::new(lines), target);
}

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

/// Volume text, lit just after a change and settling back to hint.
///
/// Blends rather than switching outright, so it reads as attention decaying
/// rather than as two states. Falls back to a hard swap when either colour is
/// a named terminal colour, for the same reason theme fades do.
fn volume_style(state: &AppState, theme: &crate::config::theme::Theme) -> Style {
  let lit = state.volume_flash();
  if lit <= 0.0 {
    return Style::default().fg(theme.hint);
  }
  let colour = crate::config::theme::blend(theme.hint, theme.active, lit);
  Style::default().fg(colour).add_modifier(Modifier::BOLD)
}

/// (title, artist line, album line, duration) for the three metadata rows.
fn item_display(
  playback: &rspotify::model::context::CurrentPlaybackContext,
) -> (String, String, String, u64) {
  match &playback.item {
    Some(PlayableItem::Track(t)) => {
      let artists = t
        .artists
        .iter()
        .map(|a| a.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
      // Album year comes free: release_date is already fetched for decade mode.
      let year = t
        .album
        .release_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .unwrap_or_default();
      let album = if year.is_empty() {
        t.album.name.clone()
      } else {
        format!("{} · {year}", t.album.name)
      };
      (
        t.name.clone(),
        artists,
        album,
        t.duration.num_milliseconds().max(0) as u64,
      )
    }
    Some(PlayableItem::Episode(e)) => (
      e.name.clone(),
      e.show.name.clone(),
      // Not `show.publisher` — Spotify removed that field and rspotify has it
      // deprecated. The episode's own date is the useful third line anyway.
      e.release_date.clone(),
      e.duration.num_milliseconds().max(0) as u64,
    ),
    Some(PlayableItem::Unknown(json)) => unknown_from_json(json),
    None => {
      let label = match playback.currently_playing_type {
        CurrentlyPlayingType::Advertisement => "(ad)",
        CurrentlyPlayingType::Unknown => "(unknown)",
        _ => "(between tracks)",
      };
      (label.to_string(), String::new(), String::new(), 0)
    }
  }
}

fn unknown_from_json(json: &Value) -> (String, String, String, u64) {
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
  (name, artists, album, duration_ms)
}

fn format_ms(ms: u64) -> String {
  let total_secs = ms / 1000;
  let minutes = total_secs / 60;
  let seconds = total_secs % 60;
  format!("{minutes}:{seconds:02}")
}

#[cfg(test)]
mod tests {
  use super::draw;
  use crate::app::{AppState, CachedCover, CoverArt, NOWPLAYING_COLS, NOWPLAYING_ROWS};
  use crate::config::user::UserConfig;
  use ratatui::{backend::TestBackend, Terminal};
  use std::sync::Arc;

  fn state() -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    AppState::new(Arc::new(cfg))
  }

  fn render(state: &AppState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| draw(f, f.area(), state)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
      .map(|y| {
        (0..buf.area.width)
          .map(|x| buf[(x, y)].symbol().to_string())
          .collect::<String>()
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn playing(uri: &str) -> rspotify::model::CurrentPlaybackContext {
    let id = uri.rsplit(':').next().unwrap();
    serde_json::from_value(serde_json::json!({
      "device": {
        "id": "d", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "T", "type": "Computer",
        "volume_percent": 50
      },
      "repeat_state": "off", "shuffle_state": true, "context": null,
      "timestamp": 1_767_225_600_000i64, "progress_ms": 84_000,
      "is_playing": true, "currently_playing_type": "track",
      "actions": { "disallows": {} },
      "item": {
        "album": {
          "album_type": "album", "artists": [], "external_urls": {},
          "href": null, "id": null, "images": [], "name": "Rumours",
          "release_date": "1977-02-04", "release_date_precision": "day",
          "album_group": null, "restrictions": null, "type": "album",
          "uri": "spotify:album:x", "total_tracks": 11
        },
        "artists": [{
          "external_urls": {}, "href": null, "id": null,
          "name": "Fleetwood Mac", "type": "artist", "uri": "spotify:artist:x"
        }],
        "disc_number": 1, "duration_ms": 254_000, "explicit": false,
        "external_ids": {}, "external_urls": {}, "href": null,
        "id": id, "is_local": false, "is_playable": true,
        "linked_from": null, "restrictions": null, "name": "Dreams",
        "popularity": 1, "preview_url": null, "track_number": 1,
        "type": "track", "uri": uri
      }
    }))
    .expect("fixture must parse")
  }

  fn art(cols: u16, rows: u16) -> CoverArt {
    CoverArt {
      cols,
      rows,
      cells: (0..cols as usize * rows as usize)
        .map(|i| {
          let n = i as u8;
          ((n, 40, 90), (n, 60, 120))
        })
        .collect(),
    }
  }

  /// Everything identifying the track shares one row now, so all of it must
  /// still reach the screen — including the album year layout D added.
  #[test]
  fn the_identity_row_carries_title_artist_and_album() {
    let mut s = state();
    s.playback = Some(playing("spotify:track:abc"));
    let out = render(&s, 90, 5);
    assert!(out.contains("Dreams"), "title:\n{out}");
    assert!(out.contains("Fleetwood Mac"), "artist:\n{out}");
    assert!(out.contains("Rumours"), "album:\n{out}");
    assert!(out.contains("1977"), "year from release_date:\n{out}");
    assert!(out.contains("1:24"), "elapsed:\n{out}");
    assert!(out.contains("4:14"), "duration:\n{out}");
    assert!(out.contains("vol 50%"), "volume:\n{out}");
    assert!(out.contains('\u{21c4}'), "shuffle indicator:\n{out}");
  }

  /// The bar must span the width left over by the cover, not sit in a narrow
  /// column. At 90 cells the cover takes 10, so the bar has ~80 to work with
  /// and a third of that is far more than the 18-wide column it replaced.
  #[test]
  fn the_progress_bar_fills_the_width_beside_the_cover() {
    let mut s = state();
    s.playback = Some(playing("spotify:track:abc"));
    let out = render(&s, 90, 5);
    let bar_row = out.lines().nth(3).expect("bar is the third content row");

    let filled = bar_row.chars().filter(|c| *c == '\u{2588}').count();
    assert!(
      filled > 20,
      "bar only filled {filled} cells — still boxed into a column?\n{out}"
    );

    // And it reaches well past where the old 18-wide column ended.
    let last = bar_row
      .char_indices()
      .filter(|(_, c)| *c == '\u{2588}')
      .map(|(i, _)| i)
      .next_back()
      .expect("some filled cells");
    assert!(last > 30, "bar stops at column {last}\n{out}");
  }

  /// Time and controls are centred, so there is space on both sides. Right
  /// alignment would leave none on the right, left none on the left.
  #[test]
  fn the_status_row_is_centred() {
    let mut s = state();
    s.playback = Some(playing("spotify:track:abc"));
    let out = render(&s, 90, 5);
    let row = out
      .lines()
      .nth(2)
      .expect("status is the second content row");

    // Centring is within the column beside the cover, so measure from there
    // rather than across the whole pane — the cover's width would otherwise
    // read as left padding and make a correct layout look off-centre.
    let inner = row.trim_matches('\u{2502}');
    let col_start = (NOWPLAYING_COLS + 2) as usize;
    let column: String = inner.chars().skip(col_start).collect();
    let lead = column.len() - column.trim_start().len();
    let trail = column.len() - column.trim_end().len();
    assert!(
      lead > 2 && trail > 2,
      "not centred: {lead} leading, {trail} trailing\n{out}"
    );
    // Centred means the two are close; allow a cell for odd widths.
    assert!(
      lead.abs_diff(trail) <= 2,
      "off-centre: {lead} vs {trail}\n{out}"
    );
  }

  /// The thumbnail draws only when its art belongs to what is playing — a
  /// stale cover outliving a track change would be worse than none.
  #[test]
  fn thumbnail_draws_only_for_the_playing_track() {
    const HALF: char = '\u{2580}';
    let mut s = state();
    s.playback = Some(playing("spotify:track:abc"));

    // Art for a different track.
    s.now_playing_cover = Some(CachedCover {
      id: "spotify:track:other".to_string(),
      art: Some(art(NOWPLAYING_COLS, NOWPLAYING_ROWS)),
    });
    let out = render(&s, 90, 5);
    assert!(!out.contains(HALF), "stale cover must not draw:\n{out}");

    // Art for the right one.
    s.now_playing_cover = Some(CachedCover {
      id: "spotify:track:abc".to_string(),
      art: Some(art(NOWPLAYING_COLS, NOWPLAYING_ROWS)),
    });
    let out = render(&s, 90, 5);
    assert!(out.contains(HALF), "matching cover must draw:\n{out}");
  }

  /// No art is a normal state and must stay silent — the metadata still has
  /// to render around the gap.
  #[test]
  fn a_missing_thumbnail_is_silent() {
    let mut s = state();
    s.playback = Some(playing("spotify:track:abc"));
    s.now_playing_cover = Some(CachedCover {
      id: "spotify:track:abc".to_string(),
      art: None,
    });
    let out = render(&s, 90, 5);
    assert!(!out.contains('\u{2580}'), "no cover drawn");
    assert!(out.contains("Dreams"), "metadata still renders:\n{out}");
  }

  /// The pane is five rows including borders; nothing may spill onto them.
  #[test]
  fn content_stays_inside_the_border() {
    let mut s = state();
    s.playback = Some(playing("spotify:track:abc"));
    s.now_playing_cover = Some(CachedCover {
      id: "spotify:track:abc".to_string(),
      art: Some(art(NOWPLAYING_COLS, NOWPLAYING_ROWS)),
    });
    let out = render(&s, 90, 5);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 5, "five rows total");
    assert!(!lines[0].contains('\u{2580}'), "cover on the top border");
    assert!(!lines[4].contains('\u{2580}'), "cover on the bottom border");
    assert!(lines[4].contains('\u{2514}'), "bottom-left corner intact");
  }
}
