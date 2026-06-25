use crate::app::{ActiveBlock, AppState, LIBRARY_ENTRIES};
use crate::ui::layout;
use ratatui::{
  layout::Rect,
  style::{Modifier, Style},
  text::Line,
  widgets::{List, ListItem, ListState},
  Frame,
};

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let block = layout::block("Library", ActiveBlock::Library, state.active_block, &theme);

  let items: Vec<ListItem> = LIBRARY_ENTRIES
    .iter()
    .map(|entry| ListItem::new(Line::raw(*entry)))
    .collect();

  let list = List::new(items)
    .block(block)
    .highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  let mut list_state = ListState::default();
  if state.active_block == ActiveBlock::Library {
    list_state.select(Some(state.library_index));
  }
  frame.render_stateful_widget(list, area, &mut list_state);
}
