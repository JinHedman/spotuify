use crate::app::{ActiveBlock, AppState};
use crate::ui::{layout, scroll};
use ratatui::{
  layout::{Constraint, Rect},
  style::{Modifier, Style},
  widgets::{Cell, Paragraph, Row, Table, TableState},
  Frame,
};

const SCROLL_MARGIN: usize = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  let title = if state.track_list_title.is_empty() {
    "Tracks".to_string()
  } else {
    format!("Tracks — {}", state.track_list_title)
  };
  // A fetch replaces the list only once it completes, so while switching from
  // one playlist to another the previous tracks stay on screen. Marking the
  // title covers that case, where an empty-state spinner never fires.
  let title = if state.track_list_loading {
    format!(
      "{title}  {}",
      crate::ui::spinner::frame(crate::ui::spinner::now_ms())
    )
  } else {
    title
  };
  let block = layout::block(&title, ActiveBlock::TrackTable, state.active_block, &theme);

  if state.track_list.is_empty() {
    let placeholder = if state.track_list_loading {
      // Paging a large playlist is many round trips; the idle hint below would
      // otherwise sit there looking like nothing had happened.
      Paragraph::new(crate::ui::spinner::line("loading tracks…", &theme))
    } else {
      Paragraph::new("Pick a playlist, search with /, or open Liked Songs.")
    };
    frame.render_widget(placeholder.block(block), area);
    return;
  }

  let header = Row::new(vec![
    Cell::from("#"),
    Cell::from("Title"),
    Cell::from("Artist"),
    Cell::from("Album"),
    Cell::from("Time"),
  ])
  .style(Style::default().fg(theme.hint).add_modifier(Modifier::BOLD));

  // Matched on URI rather than on list position, because Spotify reports what
  // is playing but not where in the context it sits. Consequence: a track
  // appearing twice in a playlist marks both rows. That is not fixable from
  // the API, and showing both is more honest than guessing one.
  //
  // Deliberately not requiring the context to match as well: Liked Songs,
  // search results and Recently Played set no `track_list_context_uri`, so a
  // context check would hide the marker in exactly the views that most need
  // it. The claim here is "this is the song that's playing", not "playback is
  // running from this list".
  let playing_uri = state.playing_uri();
  let is_playing = state.is_playing();
  let anim_ms = crate::ui::spinner::now_ms();

  let rows: Vec<Row> = state
    .track_list
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let current = crate::ui::nowplaying::is_current(t.uri.as_deref(), playing_uri.as_deref());
      let gutter = if current {
        crate::ui::nowplaying::glyph(anim_ms, is_playing).to_string()
      } else {
        // Same width as the glyph, so the column does not shift when the
        // marker appears or moves.
        format!("{:>width$}", i + 1, width = crate::ui::nowplaying::WIDTH)
      };
      let row = Row::new(vec![
        Cell::from(gutter),
        Cell::from(t.name.clone()),
        Cell::from(t.artists.clone()),
        Cell::from(t.album.clone()),
        Cell::from(format_ms(t.duration_ms)),
      ]);
      if current {
        // Foreground only, so it composes with the selection background when
        // the playing row also happens to be selected.
        row.style(Style::default().fg(theme.playing_icon))
      } else {
        row
      }
    })
    .collect();

  let widths = [
    Constraint::Length(4),
    Constraint::Percentage(32),
    Constraint::Percentage(28),
    Constraint::Percentage(28),
    Constraint::Length(6),
  ];

  let table = Table::new(rows, widths)
    .header(header)
    .block(block)
    .row_highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  let visible = (area.height as usize).saturating_sub(3);
  scroll::adjust_offset(
    state.track_list_index,
    &mut state.track_list_offset,
    visible,
    SCROLL_MARGIN,
    state.track_list.len(),
  );

  let mut table_state = TableState::default();
  if state.active_block == ActiveBlock::TrackTable {
    table_state.select(Some(state.track_list_index));
  }
  *table_state.offset_mut() = state.track_list_offset;
  frame.render_stateful_widget(table, area, &mut table_state);
  scroll::render(
    frame,
    area,
    state.track_list_offset,
    visible,
    state.track_list.len(),
    &theme,
  );
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
  use crate::app::{AppState, TrackRow};
  use crate::config::user::UserConfig;
  use ratatui::{backend::TestBackend, Terminal};
  use std::sync::Arc;

  fn row(name: &str, uri: Option<&str>) -> TrackRow {
    TrackRow {
      uri: uri.map(str::to_string),
      name: name.to_string(),
      artists: "Artist".to_string(),
      album: "Album".to_string(),
      duration_ms: 200_000,
    }
  }

  /// Builds a playback context by deserializing it, rather than assembling a
  /// dozen model structs by hand. Doubles as a check that the shape we expect
  /// from Spotify still parses.
  fn playing(uri: &str, is_playing: bool) -> rspotify::model::CurrentPlaybackContext {
    let id = uri.rsplit(':').next().unwrap();
    serde_json::from_value(serde_json::json!({
      "device": {
        "id": "dev1", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "Test", "type": "Computer",
        "volume_percent": 50
      },
      "repeat_state": "off",
      "shuffle_state": false,
      "context": null,
      "timestamp": 1_767_225_600_000i64,
      "progress_ms": 1000,
      "is_playing": is_playing,
      "currently_playing_type": "track",
      "actions": { "disallows": {} },
      "item": {
        "album": {
          "album_type": "album", "artists": [], "external_urls": {},
          "href": null, "id": null, "images": [], "name": "Album",
          "release_date": "2020", "release_date_precision": "year",
          "album_group": null, "restrictions": null, "type": "album",
          "uri": "spotify:album:x", "total_tracks": 1
        },
        "artists": [],
        "disc_number": 1,
        "duration_ms": 200_000,
        "explicit": false,
        "external_ids": {},
        "external_urls": {},
        "href": null,
        "id": id,
        "is_local": false,
        "is_playable": true,
        "linked_from": null,
        "restrictions": null,
        "name": "Beta",
        "popularity": 1,
        "preview_url": null,
        "track_number": 1,
        "type": "track",
        "uri": uri
      }
    }))
    .expect("playback fixture must deserialize")
  }

  fn state_with(list: Vec<TrackRow>) -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    let mut s = AppState::new(Arc::new(cfg));
    s.track_list = list;
    s.track_list_title = "Test".to_string();
    s
  }

  /// Renders the pane and returns every cell's character, so assertions run
  /// against what actually reaches the screen rather than the intent.
  fn render(state: &mut AppState, width: u16, height: u16) -> String {
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

  /// The row numbers must be on screen at all — establishes that the gutter
  /// column renders before asserting anything about the marker.
  #[test]
  fn gutter_column_is_rendered() {
    let mut state = state_with(vec![
      row("Alpha", Some("spotify:track:a")),
      row("Beta", Some("spotify:track:b")),
    ]);
    let out = render(&mut state, 80, 10);
    assert!(out.contains("Alpha"), "track names render:\n{out}");
    assert!(
      out.contains('1') && out.contains('2'),
      "row numbers render:\n{out}"
    );
  }

  /// The bar must appear only when the list actually overflows: on a short
  /// list its absence is the signal that there is nothing more to see.
  #[test]
  fn scrollbar_appears_only_when_the_list_overflows() {
    const THUMB: char = '\u{2503}';

    // Two rows in a ten-row pane: nothing hidden, so no bar.
    let mut short = state_with(vec![
      row("Alpha", Some("spotify:track:a")),
      row("Beta", Some("spotify:track:b")),
    ]);
    let out = render(&mut short, 60, 10);
    assert!(
      !out.contains(THUMB),
      "short list must not draw a scrollbar:\n{out}"
    );

    // Forty rows in the same pane: most are hidden, so the bar appears.
    let mut long = state_with(
      (0..40)
        .map(|i| row(&format!("Track {i}"), Some(&format!("spotify:track:{i}"))))
        .collect(),
    );
    let out = render(&mut long, 60, 10);
    assert!(
      out.contains(THUMB),
      "overflowing list must draw a scrollbar:\n{out}"
    );
  }

  /// The bar sits inside the border, never on top of it.
  #[test]
  fn scrollbar_does_not_overwrite_the_frame() {
    const THUMB: char = '\u{2503}';
    let mut state = state_with(
      (0..40)
        .map(|i| row(&format!("Track {i}"), Some(&format!("spotify:track:{i}"))))
        .collect(),
    );
    let out = render(&mut state, 60, 10);
    let lines: Vec<&str> = out.lines().collect();

    // Top and bottom rows are the frame and must be untouched by the thumb.
    assert!(!lines[0].contains(THUMB), "thumb on the top border");
    assert!(
      !lines[lines.len() - 1].contains(THUMB),
      "thumb on the bottom border"
    );
    // And the frame corners survive.
    assert!(lines[0].contains('\u{250c}'), "top-left corner intact");
  }

  /// Fails if the equalizer never reaches the buffer — whether because of a
  /// matching bug or because the column is squeezed out by overflow.
  #[test]
  fn playing_row_shows_the_equalizer() {
    let mut state = state_with(vec![
      row("Alpha", Some("spotify:track:a")),
      row("Beta", Some("spotify:track:b")),
    ]);
    state.playback = Some(playing("spotify:track:b", true));

    let out = render(&mut state, 80, 10);
    let bars = ['\u{2581}', '\u{2583}', '\u{2585}', '\u{2587}'];
    assert!(
      out.chars().any(|c| bars.contains(&c)),
      "no equalizer bar reached the screen:\n{out}"
    );
  }

  /// The regression that hides the marker in practice.
  ///
  /// `PlayableItem` is #[serde(untagged)] with an `Unknown(Value)` fallback, so
  /// a playback item whose shape has drifted from rspotify's `FullTrack` lands
  /// there instead of failing. The playbar renders those fine via its raw-JSON
  /// path, so the song looks like it is playing normally — but `playing_uri()`
  /// used to return None for Unknown, leaving the marker nothing to match and
  /// no visible reason why.
  #[tokio::test]
  async fn playing_row_is_marked_even_when_the_item_did_not_parse() {
    let mut state = state_with(vec![
      row("Alpha", Some("spotify:track:a")),
      row("Beta", Some("spotify:track:b")),
    ]);

    // Deliberately not a valid FullTrack — exactly what drops into Unknown.
    let raw = serde_json::json!({
      "device": {
        "id": "dev1", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "Test", "type": "Computer",
        "volume_percent": 50
      },
      "repeat_state": "off",
      "shuffle_state": false,
      "context": null,
      "timestamp": 1_767_225_600_000i64,
      "progress_ms": 1000,
      "is_playing": true,
      "currently_playing_type": "track",
      "actions": { "disallows": {} },
      "item": { "name": "Beta", "uri": "spotify:track:b", "some_new_field": 1 }
    });
    let playback: rspotify::model::CurrentPlaybackContext =
      serde_json::from_value(raw).expect("must parse via the Unknown fallback");
    assert!(
      playback.item.as_ref().unwrap().is_unknown(),
      "fixture must exercise the Unknown variant, not Track"
    );
    state.playback = Some(playback);

    assert_eq!(
      state.playing_uri().as_deref(),
      Some("spotify:track:b"),
      "the URI must still be recoverable from the raw JSON"
    );

    let out = render(&mut state, 80, 10);
    let bars = ['\u{2581}', '\u{2583}', '\u{2585}', '\u{2587}'];
    assert!(
      out.chars().any(|c| bars.contains(&c)),
      "marker missing for an unparsed playback item:\n{out}"
    );
  }

  /// The pane is narrower than the sum of its column constraints, which is
  /// the case the user hit. The gutter must survive it.
  #[test]
  fn equalizer_survives_a_narrow_pane() {
    let mut state = state_with(vec![row("Alpha", Some("spotify:track:a"))]);
    state.playback = Some(playing("spotify:track:a", true));

    for width in [40u16, 60, 80, 120] {
      let out = render(&mut state, width, 8);
      let bars = ['\u{2581}', '\u{2583}', '\u{2585}', '\u{2587}'];
      assert!(
        out.chars().any(|c| bars.contains(&c)),
        "equalizer missing at width {width}:\n{out}"
      );
    }
  }
}
