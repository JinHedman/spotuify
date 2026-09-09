use crate::app::{ActiveBlock, AppState, ArtistTab};
use crate::ui::{layout, scroll};
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::Line,
  widgets::{List, ListItem, ListState, Paragraph, Tabs},
  Frame,
};

const SCROLL_MARGIN: usize = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  let title = if state.artist_view.artist_name.is_empty() {
    "Artist".to_string()
  } else {
    format!("Artist — {}", state.artist_view.artist_name)
  };
  let block = layout::block(&title, ActiveBlock::ArtistView, state.active_block, &theme);
  let inner = block.inner(area);
  frame.render_widget(block, area);

  if state.artist_view.tracks.is_empty() && state.artist_view.albums.is_empty() {
    frame.render_widget(
      Paragraph::new(crate::ui::spinner::line("loading…", &theme)),
      inner,
    );
    return;
  }

  let rows = Layout::new(
    Direction::Vertical,
    [Constraint::Length(1), Constraint::Min(1)],
  )
  .split(inner);

  // The Tracks tab is only "Top tracks" when Spotify's curated endpoint
  // answered. It was removed for development-mode apps in the 2026-02-11
  // migration, so for most accounts the list is search-derived and saying
  // otherwise is what made a working fallback look broken.
  let tab_titles: Vec<Line> = ArtistTab::ALL
    .iter()
    .map(|t| match t {
      ArtistTab::Tracks if state.artist_view.tracks_are_fallback => Line::raw("Tracks"),
      other => Line::raw(other.title()),
    })
    .collect();
  let tabs = Tabs::new(tab_titles)
    .select(state.artist_view.tab.index())
    .highlight_style(
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    )
    .divider("│");
  frame.render_widget(tabs, rows[0]);

  let is_active = state.active_block == ActiveBlock::ArtistView;
  let visible = (rows[1].height as usize).saturating_sub(0).max(1);

  // An empty tab used to render as blank space, which is indistinguishable
  // from a pane that failed to load — the state that hid a 400 on the albums
  // endpoint. The other tab having content means the fetch already finished.
  let empty = match state.artist_view.tab {
    ArtistTab::Tracks => state.artist_view.tracks.is_empty(),
    ArtistTab::Albums => state.artist_view.albums.is_empty(),
  };
  if empty {
    let what = match state.artist_view.tab {
      ArtistTab::Tracks => "No tracks found for this artist.",
      ArtistTab::Albums => "No albums returned for this artist.",
    };
    frame.render_widget(
      Paragraph::new(Line::styled(what, Style::default().fg(theme.hint))),
      rows[1],
    );
    return;
  }

  match state.artist_view.tab {
    ArtistTab::Tracks => {
      let items: Vec<ListItem> = state
        .artist_view
        .tracks
        .iter()
        .map(|t| ListItem::new(format!("{}  —  {}", t.name, t.artists)))
        .collect();
      scroll::adjust_offset(
        state.artist_view.tracks_index,
        &mut state.artist_view.tracks_offset,
        visible,
        SCROLL_MARGIN,
        state.artist_view.tracks.len(),
      );
      let list = List::new(items).highlight_style(
        Style::default()
          .bg(theme.selected_bg)
          .add_modifier(Modifier::BOLD),
      );
      let mut list_state = ListState::default();
      if is_active {
        list_state.select(Some(state.artist_view.tracks_index));
      }
      *list_state.offset_mut() = state.artist_view.tracks_offset;
      frame.render_stateful_widget(list, rows[1], &mut list_state);
    }
    ArtistTab::Albums => {
      let items: Vec<ListItem> = state
        .artist_view
        .albums
        .iter()
        .map(|a| {
          let year = a
            .release_date
            .as_deref()
            .and_then(|d| d.get(..4))
            .unwrap_or("");
          if year.is_empty() {
            ListItem::new(a.name.clone())
          } else {
            ListItem::new(format!("{}  ({year})", a.name))
          }
        })
        .collect();
      scroll::adjust_offset(
        state.artist_view.albums_index,
        &mut state.artist_view.albums_offset,
        visible,
        SCROLL_MARGIN,
        state.artist_view.albums.len(),
      );
      let list = List::new(items).highlight_style(
        Style::default()
          .bg(theme.selected_bg)
          .add_modifier(Modifier::BOLD),
      );
      let mut list_state = ListState::default();
      if is_active {
        list_state.select(Some(state.artist_view.albums_index));
      }
      *list_state.offset_mut() = state.artist_view.albums_offset;
      frame.render_stateful_widget(list, rows[1], &mut list_state);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::draw;
  use crate::app::{ActiveBlock, AppState, ArtistTab, TrackRow};
  use crate::config::user::UserConfig;
  use ratatui::{backend::TestBackend, Terminal};
  use std::sync::Arc;

  fn state() -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    let mut s = AppState::new(Arc::new(cfg));
    s.active_block = ActiveBlock::ArtistView;
    s.artist_view.artist_name = "Fleetwood Mac".to_string();
    s
  }

  fn row(name: &str) -> TrackRow {
    TrackRow {
      uri: Some(format!("spotify:track:{name}")),
      name: name.to_string(),
      artists: "Fleetwood Mac".to_string(),
      album: "Rumours".to_string(),
      duration_ms: 254_000,
    }
  }

  fn render(state: &mut AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
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

  /// A tab with no rows rendered as blank space, which looks identical to a
  /// pane that failed to load — the state that hid the 400 on the albums
  /// endpoint for two rounds of debugging.
  #[test]
  fn an_empty_albums_tab_explains_itself() {
    let mut s = state();
    s.artist_view.tracks = vec![row("Dreams")];
    s.artist_view.tab = ArtistTab::Albums;

    let out = render(&mut s);
    assert!(
      out.contains("No albums"),
      "an empty albums tab must say so:\n{out}"
    );
  }

  /// Both lists empty still means loading, not an empty result.
  #[test]
  fn nothing_loaded_yet_still_shows_the_spinner() {
    let mut s = state();
    let out = render(&mut s);
    assert!(out.contains("loading"), "{out}");
    assert!(
      !out.contains("No albums"),
      "not an empty result yet:\n{out}"
    );
  }

  /// The tab claims Spotify's curated ordering only when it actually got it.
  #[test]
  fn the_tracks_tab_is_labelled_by_where_the_list_came_from() {
    let mut s = state();
    s.artist_view.tracks = vec![row("Dreams")];

    s.artist_view.tracks_are_fallback = false;
    let out = render(&mut s);
    assert!(out.contains("Top tracks"), "curated list:\n{out}");

    s.artist_view.tracks_are_fallback = true;
    let out = render(&mut s);
    assert!(
      !out.contains("Top tracks"),
      "search-derived list must not claim to be curated:\n{out}"
    );
    assert!(out.contains("Tracks"), "still labelled:\n{out}");
  }
}
