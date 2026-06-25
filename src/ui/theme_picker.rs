use crate::app::AppState;
use crate::config::presets::PRESETS;
use ratatui::{
  layout::{Alignment, Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  symbols::border,
  text::{Line, Span},
  widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
  Frame,
};

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let popup = centered(area, 40, (PRESETS.len() as u16) + 6);
  frame.render_widget(Clear, popup);

  let block = Block::new()
    .borders(Borders::ALL)
    .border_set(border::ROUNDED)
    .title(" Theme ")
    .title_alignment(Alignment::Center)
    .border_style(Style::default().fg(theme.active));
  let inner = block.inner(popup);
  frame.render_widget(block, popup);

  let rows = Layout::new(
    Direction::Vertical,
    [
      Constraint::Min(1),
      Constraint::Length(1),
      Constraint::Length(1),
    ],
  )
  .split(inner);

  let items: Vec<ListItem> = PRESETS
    .iter()
    .map(|p| {
      // Small color swatch in the active color of each preset, so you get a
      // preview of each accent before you commit.
      let swatch_color = p.theme().active;
      ListItem::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(swatch_color)),
        Span::raw(p.name),
      ]))
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
  list_state.select(Some(state.theme_picker_index));
  frame.render_stateful_widget(list, rows[0], &mut list_state);

  let divider = Paragraph::new(Line::from(Span::styled(
    "─".repeat(inner.width as usize),
    Style::default().fg(theme.inactive),
  )));
  frame.render_widget(divider, rows[1]);

  let hint = Line::from(vec![
    Span::styled(
      "Enter",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    ),
    Span::styled(" apply  ", Style::default().fg(theme.hint)),
    Span::styled(
      "Esc",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    ),
    Span::styled(" cancel", Style::default().fg(theme.hint)),
  ]);
  frame.render_widget(Paragraph::new(hint).alignment(Alignment::Center), rows[2]);
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
