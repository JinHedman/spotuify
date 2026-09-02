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
    "Saved podcasts",
    ActiveBlock::SavedShows,
    state.active_block,
    &theme,
  );

  if state.saved_shows.is_empty() {
    frame.render_widget(
      Paragraph::new(crate::ui::spinner::line("loading…", &theme)).block(block),
      area,
    );
    return;
  }

  let items: Vec<ListItem> = state
    .saved_shows
    .iter()
    .map(|s| {
      #[allow(deprecated)]
      let publisher = &s.show.publisher;
      let subtitle = if publisher.is_empty() {
        String::new()
      } else {
        format!("  —  {publisher}")
      };
      ListItem::new(Line::raw(format!("{}{subtitle}", s.show.name)))
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
    state.saved_shows_index,
    &mut state.saved_shows_offset,
    visible,
    SCROLL_MARGIN,
    state.saved_shows.len(),
  );

  let mut list_state = ListState::default();
  if state.active_block == ActiveBlock::SavedShows {
    list_state.select(Some(state.saved_shows_index));
  }
  *list_state.offset_mut() = state.saved_shows_offset;
  frame.render_stateful_widget(list, area, &mut list_state);
}
