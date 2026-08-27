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
  // Say so when the owner filter is hiding entries, so a short list never
  // looks like missing data.
  let title = if state.playlists_hidden > 0 {
    format!(
      "Playlists ({} yours, {} hidden)",
      state.playlists.len(),
      state.playlists_hidden
    )
  } else {
    "Playlists".to_string()
  };
  let block = layout::block(&title, ActiveBlock::MyPlaylists, state.active_block, &theme);

  if state.playlists.is_empty() {
    let placeholder = Paragraph::new("(loading…)").block(block);
    frame.render_widget(placeholder, area);
    return;
  }

  let items: Vec<ListItem> = state
    .playlists
    .iter()
    .map(|p| ListItem::new(Line::raw(p.name.clone())))
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
    state.playlists_index,
    &mut state.playlists_offset,
    visible,
    SCROLL_MARGIN,
    state.playlists.len(),
  );

  let mut list_state = ListState::default();
  if state.active_block == ActiveBlock::MyPlaylists {
    list_state.select(Some(state.playlists_index));
  }
  *list_state.offset_mut() = state.playlists_offset;
  frame.render_stateful_widget(list, area, &mut list_state);
}
