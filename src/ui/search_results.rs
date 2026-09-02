use crate::app::{ActiveBlock, AppState, SearchResults, SearchTab};
use crate::ui::layout;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::Line,
  widgets::{List, ListItem, ListState, Paragraph, Tabs},
  Frame,
};
use rspotify::prelude::Id;

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
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
  let (items, selected) = make_items(&state.search_results, state.search_tab, is_active);

  let list = List::new(items).highlight_style(
    Style::default()
      .bg(theme.selected_bg)
      .add_modifier(Modifier::BOLD),
  );
  let mut list_state = ListState::default();
  list_state.select(selected);
  frame.render_stateful_widget(list, rows[1], &mut list_state);
}

fn make_items(
  results: &SearchResults,
  tab: SearchTab,
  active: bool,
) -> (Vec<ListItem<'static>>, Option<usize>) {
  match tab {
    SearchTab::Tracks => (
      results
        .tracks
        .iter()
        .map(|t| {
          let artists = t
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
          ListItem::new(format!("{}  —  {}", t.name, artists))
        })
        .collect(),
      active.then_some(results.tracks_index),
    ),
    SearchTab::Albums => (
      results
        .albums
        .iter()
        .map(|a| {
          let artists = a
            .artists
            .iter()
            .map(|x| x.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
          ListItem::new(format!("{}  —  {}", a.name, artists))
        })
        .collect(),
      active.then_some(results.albums_index),
    ),
    SearchTab::Artists => (
      results
        .artists
        .iter()
        .map(|a| ListItem::new(a.name.clone()))
        .collect(),
      active.then_some(results.artists_index),
    ),
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
