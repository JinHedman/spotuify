use crate::app::{ActiveBlock, AppState};
use crate::ui::{layout, scroll};
use ratatui::{
  layout::{Constraint, Rect},
  style::{Modifier, Style},
  widgets::{Cell, Paragraph, Row, Table, TableState},
  Frame,
};

const SCROLL_MARGIN: usize = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  let title = if state.show_episodes_title.is_empty() {
    "Episodes".to_string()
  } else {
    format!("Episodes — {}", state.show_episodes_title)
  };
  let block = layout::block(
    &title,
    ActiveBlock::ShowEpisodes,
    state.active_block,
    &theme,
  );

  if state.show_episodes.is_empty() {
    frame.render_widget(
      Paragraph::new(crate::ui::spinner::line("loading…", &theme)).block(block),
      area,
    );
    return;
  }

  let header = Row::new(vec![
    Cell::from("Date"),
    Cell::from("Episode"),
    Cell::from("Time"),
  ])
  .style(Style::default().fg(theme.hint).add_modifier(Modifier::BOLD));

  let rows: Vec<Row> = state
    .show_episodes
    .iter()
    .map(|e| {
      Row::new(vec![
        Cell::from(e.release_date.clone()),
        Cell::from(e.name.clone()),
        Cell::from(format_ms(e.duration.num_milliseconds().max(0) as u64)),
      ])
    })
    .collect();

  let widths = [
    Constraint::Length(12),
    Constraint::Min(20),
    Constraint::Length(6),
  ];

  let table = Table::new(rows, widths)
    .header(header)
    .block(block)
    .row_highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  let visible = (area.height as usize).saturating_sub(3);
  scroll::adjust_offset(
    state.show_episodes_index,
    &mut state.show_episodes_offset,
    visible,
    SCROLL_MARGIN,
    state.show_episodes.len(),
  );

  let mut table_state = TableState::default();
  if state.active_block == ActiveBlock::ShowEpisodes {
    table_state.select(Some(state.show_episodes_index));
  }
  *table_state.offset_mut() = state.show_episodes_offset;
  frame.render_stateful_widget(table, area, &mut table_state);
}

fn format_ms(ms: u64) -> String {
  let total_secs = ms / 1000;
  let minutes = total_secs / 60;
  let seconds = total_secs % 60;
  format!("{minutes}:{seconds:02}")
}
