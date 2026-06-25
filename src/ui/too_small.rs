use crate::config::theme::Theme;
use ratatui::{
  layout::{Alignment, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::Paragraph,
  Frame,
};

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
  let lines = vec![
    Line::from(Span::styled(
      "Terminal too small",
      Style::default()
        .fg(theme.active)
        .add_modifier(Modifier::BOLD),
    )),
    Line::raw(""),
    Line::from(Span::styled(
      format!("{}×{} — resize to continue", area.width, area.height),
      Style::default().fg(theme.hint),
    )),
  ];

  let body = Paragraph::new(lines).alignment(Alignment::Center);

  let top = area.height.saturating_sub(3) / 2;
  let centered = Rect {
    x: area.x,
    y: area.y + top,
    width: area.width,
    height: area.height.saturating_sub(top),
  };
  frame.render_widget(body, centered);
}
