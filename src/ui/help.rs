use crate::app::AppState;
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
  ("  k / j", "scroll this help"),
];

/// Rows the overlay leaves free around itself, so it never sits flush against
/// the terminal edge.
const MARGIN: u16 = 4;
/// Cap so the overlay stays a dialog rather than becoming a full-screen page
/// on a very tall terminal. Sized to fit the whole map when there is room.
const MAX_HEIGHT: u16 = 40;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  // Take as much height as the terminal allows, up to the whole list plus
  // borders. Previously fixed at 30, which clipped the last seven entries —
  // including the newest bindings — with nothing on screen to suggest that
  // content had been cut off.
  let wanted = HELP.len() as u16 + 2;
  let height = wanted
    .min(MAX_HEIGHT)
    .min(area.height.saturating_sub(MARGIN));
  let popup = centered(area, 60, height);
  frame.render_widget(Clear, popup);

  let visible = popup.height.saturating_sub(2);
  let max_scroll = (HELP.len() as u16).saturating_sub(visible);
  // Clamp here: draw is the only place the visible height is known, and the
  // handler increments blindly.
  state.help_scroll = state.help_scroll.min(max_scroll);
  let scroll = state.help_scroll;

  let title = if max_scroll == 0 {
    " Help  (? or Esc to close) ".to_string()
  } else {
    format!(
      " Help  ({}-{} of {} · k/j to scroll · ? or Esc to close) ",
      scroll + 1,
      (scroll + visible).min(HELP.len() as u16),
      HELP.len()
    )
  };

  let block = Block::new()
    .borders(Borders::ALL)
    .title(title)
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

  frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
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
