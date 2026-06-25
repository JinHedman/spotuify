use crate::app::AppState;
use ratatui::{
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, Paragraph},
  Frame,
};

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let Some(dialog) = state.dialog.as_ref() else {
    return;
  };

  let theme = state.theme;
  let popup = centered(area, 60, 7);
  frame.render_widget(Clear, popup);

  let block = Block::new()
    .borders(Borders::ALL)
    .title(" Confirm ")
    .border_style(Style::default().fg(theme.active));
  let inner = block.inner(popup);
  frame.render_widget(block, popup);

  let rows = Layout::new(
    Direction::Vertical,
    [Constraint::Min(1), Constraint::Length(1)],
  )
  .split(inner);

  let message = Paragraph::new(dialog.message.clone()).alignment(Alignment::Center);
  frame.render_widget(message, rows[0]);

  let hint = Line::from(vec![
    Span::styled(
      "Y",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    ),
    Span::styled("es", Style::default().fg(theme.hint)),
    Span::raw("  "),
    Span::styled(
      "N",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    ),
    Span::styled("o", Style::default().fg(theme.hint)),
  ]);
  let hint_paragraph = Paragraph::new(hint).alignment(Alignment::Center);
  frame.render_widget(hint_paragraph, rows[1]);
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
