use crate::app::AppState;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::Line,
  widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
  Frame,
};

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let popup = centered(area, 60, 15);
  frame.render_widget(Clear, popup);

  let block = Block::new()
    .borders(Borders::ALL)
    .title(" Select device  (Enter = transfer, Esc = close) ")
    .border_style(Style::default().fg(theme.active));
  let inner = block.inner(popup);
  frame.render_widget(block, popup);

  if state.devices.is_empty() {
    frame.render_widget(
      Paragraph::new("No devices found. Start Spotify somewhere, then press d again."),
      inner,
    );
    return;
  }

  let items: Vec<ListItem> = state
    .devices
    .iter()
    .map(|d| {
      let active = if d.is_active { "●" } else { " " };
      let vol = d
        .volume_percent
        .map(|v| format!("  {v}%"))
        .unwrap_or_default();
      ListItem::new(Line::raw(format!(
        "{active} {}  ({:?}){vol}",
        d.name, d._type
      )))
    })
    .collect();

  let list = List::new(items)
    .highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  let mut list_state = ListState::default();
  list_state.select(Some(state.devices_index));
  frame.render_stateful_widget(list, inner, &mut list_state);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
  let width = width.min(area.width);
  let height = height.min(area.height);
  let vertical = Layout::new(
    Direction::Vertical,
    [
      Constraint::Length((area.height - height) / 2),
      Constraint::Length(height),
      Constraint::Min(0),
    ],
  )
  .split(area);
  let horizontal = Layout::new(
    Direction::Horizontal,
    [
      Constraint::Length((area.width - width) / 2),
      Constraint::Length(width),
      Constraint::Min(0),
    ],
  )
  .split(vertical[1]);
  horizontal[1]
}
