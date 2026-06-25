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
    "Followed artists",
    ActiveBlock::FollowedArtists,
    state.active_block,
    &theme,
  );

  if state.followed_artists.is_empty() {
    frame.render_widget(Paragraph::new("(loading…)").block(block), area);
    return;
  }

  let items: Vec<ListItem> = state
    .followed_artists
    .iter()
    .map(|a| ListItem::new(Line::raw(a.name.clone())))
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
    state.followed_artists_index,
    &mut state.followed_artists_offset,
    visible,
    SCROLL_MARGIN,
  );

  let mut list_state = ListState::default();
  if state.active_block == ActiveBlock::FollowedArtists {
    list_state.select(Some(state.followed_artists_index));
  }
  *list_state.offset_mut() = state.followed_artists_offset;
  frame.render_stateful_widget(list, area, &mut list_state);
}
