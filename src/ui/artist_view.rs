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

  let tab_titles: Vec<Line> = ArtistTab::ALL
    .iter()
    .map(|t| Line::raw(t.title()))
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
