use crate::app::AppState;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
  Frame,
};
use rspotify::model::PlayableItem;

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let popup = centered(area, 80, 28);
  frame.render_widget(Clear, popup);

  let block = Block::new()
    .borders(Borders::ALL)
    .title(" Queue  (Q or Esc to close) ")
    .border_style(Style::default().fg(theme.active));
  let inner = block.inner(popup);
  frame.render_widget(block, popup);

  let rows = Layout::new(
    Direction::Vertical,
    [Constraint::Length(3), Constraint::Min(1)],
  )
  .split(inner);

  let now = state
    .queue_current
    .as_ref()
    .map(display_item)
    .unwrap_or_else(|| "(nothing playing)".to_string());
  let header = Paragraph::new(vec![
    Line::from(Span::styled(
      "Now playing",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    )),
    Line::raw(format!("  {now}")),
    Line::from(Span::styled(
      "Upcoming",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    )),
  ]);
  frame.render_widget(header, rows[0]);

  if state.queue_items.is_empty() {
    frame.render_widget(Paragraph::new("  (queue is empty)"), rows[1]);
    return;
  }

  let items: Vec<ListItem> = state
    .queue_items
    .iter()
    .map(|item| ListItem::new(Line::raw(display_item(item))))
    .collect();

  let list = List::new(items).highlight_style(
    Style::default()
      .bg(theme.selected_bg)
      .add_modifier(Modifier::BOLD),
  );
  let mut list_state = ListState::default();
  list_state.select(Some(state.queue_index));
  frame.render_stateful_widget(list, rows[1], &mut list_state);
}

fn display_item(item: &PlayableItem) -> String {
  match item {
    PlayableItem::Track(t) => {
      let artists = t
        .artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
      format!("{}  —  {artists}", t.name)
    }
    PlayableItem::Episode(e) => format!("{}  —  {}", e.name, e.show.name),
    PlayableItem::Unknown(_) => "(unrecognized)".to_string(),
  }
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
