use crate::config::theme::Theme;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, Paragraph},
  Frame,
};

const HELP: &[(&str, &str)] = &[
  ("Navigation", ""),
  ("  Ctrl+h / l", "focus pane left / right"),
  ("  Ctrl+j / k", "focus pane up / down (inverted)"),
  ("  k / j  or  ↓ / ↑", "move selection (inverted)"),
  ("  K / J", "move selection by 5 (inverted)"),
  ("  g / G", "top / bottom of list"),
  ("  Enter / l", "activate / open selected item"),
  ("  h / q / b / Backspace", "back"),
  ("  Esc / Ctrl+C", "quit"),
  ("", ""),
  ("Search", ""),
  ("  /", "open search input"),
  ("  Enter (in input)", "submit query"),
  ("  Tab / Shift+Tab", "cycle result tabs"),
  ("  ← / →", "cycle result tabs (alt)"),
  ("", ""),
  ("Playback", ""),
  ("  Space", "play / pause"),
  ("  n / p", "next / previous track"),
  ("  + / -", "volume up / down"),
  ("  [ / ]", "seek ±5s"),
  ("  r", "refresh current playback"),
  ("  s", "toggle save of current track"),
  ("  S", "toggle save of current album"),
  ("  f", "toggle follow of current artist"),
  ("  d", "select device"),
  ("  Q", "show playback queue"),
  ("  A", "add selected track/episode to queue"),
  ("  D", "delete playlist (confirm)"),
  ("  z", "toggle shuffle"),
  ("  R", "cycle repeat (off / all / one)"),
  ("  t", "change theme"),
  ("", ""),
  ("Help", ""),
  ("  ?", "toggle this help"),
];

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
  let popup = centered(area, 56, 30);
  frame.render_widget(Clear, popup);

  let block = Block::new()
    .borders(Borders::ALL)
    .title(" Help  (press ? or Esc to close) ")
    .border_style(Style::default().fg(theme.active));
  let inner = block.inner(popup);
  frame.render_widget(block, popup);

  let lines: Vec<Line> = HELP
    .iter()
    .map(|(k, v)| {
      if v.is_empty() {
        Line::from(Span::styled(
          *k,
          Style::default()
            .fg(theme.active)
            .add_modifier(Modifier::BOLD),
        ))
      } else {
        Line::from(vec![
          Span::styled(
            format!("{k:<22}"),
            Style::default().add_modifier(Modifier::BOLD),
          ),
          Span::styled(*v, Style::default().fg(theme.hint)),
        ])
      }
    })
    .collect();

  frame.render_widget(Paragraph::new(lines), inner);
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
