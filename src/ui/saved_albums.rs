use crate::app::{ActiveBlock, AppState};
use crate::ui::{layout, scroll};
use ratatui::{
  layout::Rect,
  style::{Modifier, Style},
  text::Line,
  widgets::{List, ListItem, ListState, Paragraph},
  Frame,
};

const SCROLL_MARGIN: usize = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  let block = layout::block(
    "Saved albums",
    ActiveBlock::SavedAlbums,
    state.active_block,
    &theme,
  );

  if state.saved_albums.is_empty() {
    frame.render_widget(
      Paragraph::new(crate::ui::spinner::line("loading…", &theme)).block(block),
      area,
    );
    return;
  }

  let items: Vec<ListItem> = state
    .saved_albums
    .iter()
    .map(|sa| {
      let artists = sa
        .album
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
      ListItem::new(Line::raw(format!("{}  —  {artists}", sa.album.name)))
    })
    .collect();

  let list = List::new(items)
    .block(block)
    .highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  let visible = (area.height as usize).saturating_sub(2);
  scroll::adjust_offset(
    state.saved_albums_index,
    &mut state.saved_albums_offset,
    visible,
    SCROLL_MARGIN,
    state.saved_albums.len(),
  );

  let mut list_state = ListState::default();
  if state.active_block == ActiveBlock::SavedAlbums {
    list_state.select(Some(state.saved_albums_index));
  }
  *list_state.offset_mut() = state.saved_albums_offset;
  frame.render_stateful_widget(list, area, &mut list_state);
}
