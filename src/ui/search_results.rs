use crate::app::{ActiveBlock, AppState, SearchTab};
use crate::ui::{layout, scroll};
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::Line,
  widgets::{Cell, Paragraph, Row, Table, TableState, Tabs},
  Frame,
};
use rspotify::prelude::Id;

const SCROLL_MARGIN: usize = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  let block = layout::block(
    "Search results",
    ActiveBlock::SearchResults,
    state.active_block,
    &theme,
  );
  let inner = block.inner(area);
  frame.render_widget(block, area);

  // The first search has no results to fall back on, so show progress in the
  // body. A re-search keeps the previous results visible and is marked on the
  // tab row instead, further down.
  if state.search_loading && !state.has_searched {
    frame.render_widget(
      Paragraph::new(crate::ui::spinner::line("searching…", &theme)),
      inner,
    );
    return;
  }

  if !state.has_searched {
    frame.render_widget(Paragraph::new("Press / to search Spotify."), inner);
    return;
  }

  let rows = Layout::new(
    Direction::Vertical,
    [Constraint::Length(1), Constraint::Min(1)],
  )
  .split(inner);

  let tab_titles: Vec<Line> = SearchTab::ALL
    .iter()
    .map(|t| Line::raw(t.title()))
    .collect();
  let tabs = Tabs::new(tab_titles)
    .select(state.search_tab.index())
    .highlight_style(
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    )
    .divider("│");

  // Re-searching replaces results only on completion, so the old ones stay on
  // screen. Mark the tab row rather than blanking the pane.
  if state.search_loading {
    let split = Layout::new(
      Direction::Horizontal,
      [Constraint::Min(1), Constraint::Length(2)],
    )
    .split(rows[0]);
    frame.render_widget(tabs, split[0]);
    frame.render_widget(
      Paragraph::new(Line::styled(
        crate::ui::spinner::frame(crate::ui::spinner::now_ms()),
        Style::default().fg(theme.active),
      )),
      split[1],
    );
  } else {
    frame.render_widget(tabs, rows[0]);
  }

  let is_active = state.active_block == ActiveBlock::SearchResults;
  let (header, widths, body) = build(state, &theme);

  if body.is_empty() {
    frame.render_widget(
      Paragraph::new(Line::styled("No results.", Style::default().fg(theme.hint))),
      rows[1],
    );
    return;
  }

  let table = Table::new(body, widths)
    .header(Row::new(header).style(Style::default().fg(theme.hint).add_modifier(Modifier::BOLD)))
    .row_highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  // Row 1 of the pane is the tab strip and row 2 the table header, so the
  // visible body is what remains after both — plus the pane's own borders,
  // which `rows[1]` has already excluded.
  let total = tab_len(state);
  let (index, offset) = tab_cursor(state);
  let visible = (rows[1].height as usize).saturating_sub(1);
  scroll::adjust_offset(index, offset, visible, SCROLL_MARGIN, total);
  let offset = *offset;

  let mut table_state = TableState::default();
  if is_active {
    table_state.select(Some(index));
  }
  *table_state.offset_mut() = offset;
  frame.render_stateful_widget(table, rows[1], &mut table_state);

  // The pane border is one row above and below `rows[1]`, so hand the
  // scrollbar the bordered area rather than the inner one.
  scroll::render(frame, area, offset, visible, total, &theme);
}

/// Number of rows in the active tab.
fn tab_len(state: &AppState) -> usize {
  match state.search_tab {
    SearchTab::Tracks => state.search_results.tracks.len(),
    SearchTab::Albums => state.search_results.albums.len(),
    SearchTab::Artists => state.search_results.artists.len(),
  }
}

/// Selected index and a mutable handle to the active tab's scroll offset.
fn tab_cursor(state: &mut AppState) -> (usize, &mut usize) {
  match state.search_tab {
    SearchTab::Tracks => (
      state.search_results.tracks_index,
      &mut state.search_results.tracks_offset,
    ),
    SearchTab::Albums => (
      state.search_results.albums_index,
      &mut state.search_results.albums_offset,
    ),
    SearchTab::Artists => (
      state.search_results.artists_index,
      &mut state.search_results.artists_offset,
    ),
  }
}

type Built<'a> = (Vec<&'static str>, Vec<Constraint>, Vec<Row<'a>>);

/// Columns for the active tab, shaped like the track table so the two panes
/// read the same way: a fixed gutter, then the identifying columns, then a
/// short right-aligned field.
fn build<'a>(state: &AppState, theme: &crate::config::theme::Theme) -> Built<'a> {
  let gutter = Constraint::Length(4);
  match state.search_tab {
    SearchTab::Tracks => {
      // Same marker the track table uses, so a search result for the playing
      // track is identifiable without switching panes.
      let playing_uri = state.playing_uri();
      let is_playing = state.is_playing();
      let anim_ms = crate::ui::spinner::now_ms();

      let rows = state
        .search_results
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
          let uri = t.id.as_ref().map(|id| id.uri());
          let current = crate::ui::nowplaying::is_current(uri.as_deref(), playing_uri.as_deref());
          let lead = if current {
            crate::ui::nowplaying::glyph(anim_ms, is_playing).to_string()
          } else {
            format!("{:>width$}", i + 1, width = crate::ui::nowplaying::WIDTH)
          };
          let artists = t
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
          let row = Row::new(vec![
            Cell::from(lead),
            Cell::from(t.name.clone()),
            Cell::from(artists),
            Cell::from(t.album.name.clone()),
            Cell::from(crate::ui::format::ms(
              t.duration.num_milliseconds().max(0) as u64
            )),
          ]);
          if current {
            row.style(Style::default().fg(theme.playing_icon))
          } else {
            row
          }
        })
        .collect();
      (
        vec!["#", "Title", "Artist", "Album", "Time"],
        vec![
          gutter,
          Constraint::Percentage(32),
          Constraint::Percentage(28),
          Constraint::Percentage(28),
          Constraint::Length(6),
        ],
        rows,
      )
    }
    SearchTab::Albums => {
      let rows = state
        .search_results
        .albums
        .iter()
        .enumerate()
        .map(|(i, a)| {
          let artists = a
            .artists
            .iter()
            .map(|x| x.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
          // Year only: release_date precision varies between year, month and
          // day, so the first four characters are the one safe slice.
          let year = a
            .release_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .unwrap_or("")
            .to_string();
          Row::new(vec![
            Cell::from(format!(
              "{:>width$}",
              i + 1,
              width = crate::ui::nowplaying::WIDTH
            )),
            Cell::from(a.name.clone()),
            Cell::from(artists),
            Cell::from(year),
          ])
        })
        .collect();
      (
        vec!["#", "Album", "Artist", "Year"],
        vec![
          gutter,
          Constraint::Percentage(48),
          Constraint::Percentage(40),
          Constraint::Length(6),
        ],
        rows,
      )
    }
    SearchTab::Artists => {
      let rows = state
        .search_results
        .artists
        .iter()
        .enumerate()
        .map(|(i, a)| {
          Row::new(vec![
            Cell::from(format!(
              "{:>width$}",
              i + 1,
              width = crate::ui::nowplaying::WIDTH
            )),
            Cell::from(a.name.clone()),
          ])
        })
        .collect();
      (vec!["#", "Artist"], vec![gutter, Constraint::Min(20)], rows)
    }
  }
}

pub fn selected_track_uri(state: &AppState) -> Option<String> {
  let idx = state.search_results.tracks_index;
  state
    .search_results
    .tracks
    .get(idx)
    .and_then(|t| t.id.as_ref().map(|id| id.uri()))
}

pub fn all_track_uris(state: &AppState) -> Vec<String> {
  state
    .search_results
    .tracks
    .iter()
    .filter_map(|t| t.id.as_ref().map(|id| id.uri()))
    .collect()
}

pub fn selected_album(state: &AppState) -> Option<(String, String)> {
  let idx = state.search_results.albums_index;
  state.search_results.albums.get(idx).and_then(|a| {
    a.id
      .as_ref()
      .map(|id| (id.id().to_string(), a.name.clone()))
  })
}

pub fn selected_artist(state: &AppState) -> Option<(String, String)> {
  let idx = state.search_results.artists_index;
  state
    .search_results
    .artists
    .get(idx)
    .map(|a| (a.id.id().to_string(), a.name.clone()))
}

#[cfg(test)]
mod tests {
  use super::draw;
  use crate::app::{ActiveBlock, AppState, SearchTab};
  use crate::config::user::UserConfig;
  use ratatui::{backend::TestBackend, Terminal};
  use std::sync::Arc;

  fn state() -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    let mut s = AppState::new(Arc::new(cfg));
    s.has_searched = true;
    s.active_block = ActiveBlock::SearchResults;
    s
  }

  fn track(name: &str, id: &str) -> rspotify::model::FullTrack {
    serde_json::from_value(serde_json::json!({
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
      "linked_from": null, "restrictions": null, "name": name,
      "popularity": 1, "preview_url": null, "track_number": 1,
      "type": "track", "uri": format!("spotify:track:{id}")
    }))
    .expect("track fixture must parse")
  }

  fn render(state: &mut AppState, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
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

  /// The change: results are a table with the same columns as the track
  /// table, not a single "Name — Artists" string per row.
  #[test]
  fn tracks_render_as_a_table_with_columns() {
    let mut s = state();
    s.search_results.tracks = vec![track("Dreams", "aaaaaaaaaaaaaaaaaaaaaa")];
    let out = render(&mut s, 90, 10);

    for column in ["#", "Title", "Artist", "Album", "Time"] {
      assert!(out.contains(column), "missing column {column}:\n{out}");
    }
    assert!(out.contains("Dreams"), "title:\n{out}");
    assert!(out.contains("Fleetwood Mac"), "artist:\n{out}");
    assert!(out.contains("Rumours"), "album:\n{out}");
    assert!(out.contains("4:14"), "duration:\n{out}");
    assert!(
      !out.contains("Dreams  —  Fleetwood Mac"),
      "old single-string row still rendered:\n{out}"
    );
  }

  /// A search result for the playing track carries the same marker the track
  /// table uses, so it is identifiable without switching panes.
  #[test]
  fn the_playing_track_is_marked_in_results() {
    let bars = ['\u{2581}', '\u{2583}', '\u{2585}', '\u{2587}'];
    let mut s = state();
    s.search_results.tracks = vec![
      track("Dreams", "aaaaaaaaaaaaaaaaaaaaaa"),
      track("The Chain", "bbbbbbbbbbbbbbbbbbbbbb"),
    ];

    let out = render(&mut s, 90, 10);
    assert!(
      !out.chars().any(|c| bars.contains(&c)),
      "nothing playing, so no marker:\n{out}"
    );

    let raw = serde_json::json!({
      "device": {
        "id": "d", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "T", "type": "Computer",
        "volume_percent": 50
      },
      "repeat_state": "off", "shuffle_state": false, "context": null,
      "timestamp": 1_767_225_600_000i64, "progress_ms": 0, "is_playing": true,
      "currently_playing_type": "track", "actions": { "disallows": {} },
      "item": {
        "name": "The Chain",
        "uri": "spotify:track:bbbbbbbbbbbbbbbbbbbbbb",
        "album": {}
      }
    });
    s.playback = Some(serde_json::from_value(raw).expect("playback fixture"));
    let out = render(&mut s, 90, 10);
    assert!(
      out.chars().any(|c| bars.contains(&c)),
      "playing track must be marked:\n{out}"
    );
  }

  /// Same rule as every other list: a bar only when the list overflows.
  #[test]
  fn a_scrollbar_appears_only_when_results_overflow() {
    const THUMB: char = '\u{2503}';

    let mut few = state();
    few.search_results.tracks = (0..2)
      .map(|i| track(&format!("Track {i}"), &format!("{i:a>22}")))
      .collect();
    let out = render(&mut few, 70, 12);
    assert!(!out.contains(THUMB), "short list draws no bar:\n{out}");

    let mut many = state();
    many.search_results.tracks = (0..40)
      .map(|i| track(&format!("Track {i}"), &format!("{i:a>22}")))
      .collect();
    let out = render(&mut many, 70, 12);
    assert!(out.contains(THUMB), "overflowing list draws one:\n{out}");
  }

  /// Each tab keeps its own offset, so switching tabs does not reset the
  /// others' scroll position.
  #[test]
  fn each_tab_scrolls_independently() {
    let mut s = state();
    s.search_results.tracks = (0..40)
      .map(|i| track(&format!("Track {i}"), &format!("{i:a>22}")))
      .collect();
    s.search_results.tracks_index = 39;
    render(&mut s, 70, 12);
    let track_offset = s.search_results.tracks_offset;
    assert!(track_offset > 0, "tracks scrolled to reach row 40");

    s.search_tab = SearchTab::Artists;
    render(&mut s, 70, 12);
    assert_eq!(
      s.search_results.tracks_offset, track_offset,
      "the tracks offset must survive a tab switch"
    );
  }

  #[test]
  fn an_empty_tab_says_so() {
    let mut s = state();
    let out = render(&mut s, 70, 8);
    assert!(out.contains("No results"), "{out}");
  }
}
