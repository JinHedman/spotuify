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
  let title = if state.track_list_title.is_empty() {
    "Tracks".to_string()
  } else {
    format!("Tracks — {}", state.track_list_title)
  };
  let block = layout::block(&title, ActiveBlock::TrackTable, state.active_block, &theme);

  if state.track_list.is_empty() {
    let placeholder =
      Paragraph::new("Pick a playlist, search with /, or open Liked Songs.").block(block);
    frame.render_widget(placeholder, area);
    return;
  }

  let header = Row::new(vec![
    Cell::from("#"),
    Cell::from("Title"),
    Cell::from("Artist"),
    Cell::from("Album"),
    Cell::from("Time"),
  ])
  .style(Style::default().fg(theme.hint).add_modifier(Modifier::BOLD));

  let rows: Vec<Row> = state
    .track_list
    .iter()
    .enumerate()
    .map(|(i, t)| {
      Row::new(vec![
        Cell::from(format!("{:>3}", i + 1)),
        Cell::from(t.name.clone()),
        Cell::from(t.artists.clone()),
        Cell::from(t.album.clone()),
        Cell::from(format_ms(t.duration_ms)),
      ])
    })
    .collect();

  let widths = [
    Constraint::Length(4),
    Constraint::Percentage(32),
    Constraint::Percentage(28),
    Constraint::Percentage(28),
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
    state.track_list_index,
    &mut state.track_list_offset,
    visible,
    SCROLL_MARGIN,
  );

  let mut table_state = TableState::default();
  if state.active_block == ActiveBlock::TrackTable {
    table_state.select(Some(state.track_list_index));
  }
  *table_state.offset_mut() = state.track_list_offset;
  frame.render_stateful_widget(table, area, &mut table_state);
}

fn format_ms(ms: u64) -> String {
  let total_secs = ms / 1000;
  let minutes = total_secs / 60;
  let seconds = total_secs % 60;
  format!("{minutes}:{seconds:02}")
}
