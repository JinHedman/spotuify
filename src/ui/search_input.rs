use crate::app::{ActiveBlock, AppState};
use crate::ui::layout;
use ratatui::{
  layout::Rect,
  style::Style,
  text::{Line, Span},
  widgets::Paragraph,
  Frame,
};

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let block = layout::block(
    "Search  (/)",
    ActiveBlock::SearchInput,
    state.active_block,
    &theme,
  );

  let cursor = if state.active_block == ActiveBlock::SearchInput {
    "▎"
  } else {
    ""
  };
  let prompt = Line::from(vec![
    Span::styled("❯ ", Style::default().fg(theme.active)),
    Span::raw(state.search_query.clone()),
    Span::styled(cursor, Style::default().fg(theme.active)),
  ]);
  frame.render_widget(Paragraph::new(prompt).block(block), area);
}
