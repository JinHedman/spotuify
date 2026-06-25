use crate::config::theme::Theme;
use ratatui::{
  layout::{Alignment, Rect},
  style::Style,
  text::{Line, Span},
  widgets::Paragraph,
  Frame,
};

const ENTRIES: &[(&str, &str)] = &[
  ("?", "help"),
  ("C-hjkl", "panes"),
  ("Tab", "tabs"),
  ("/", "search"),
  ("d", "device"),
  ("Q", "queue"),
  ("s", "save"),
  ("t", "theme"),
  ("Space", "play/pause"),
  ("b", "back"),
  ("q", "quit"),
];

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
  let mut spans: Vec<Span> = Vec::with_capacity(ENTRIES.len() * 3);
  for (i, (key, label)) in ENTRIES.iter().enumerate() {
    if i > 0 {
      spans.push(Span::styled("  ·  ", Style::default().fg(theme.hint)));
    }
    spans.push(Span::styled(*key, Style::default().fg(theme.active)));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(*label, Style::default().fg(theme.hint)));
  }
  let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
  frame.render_widget(paragraph, area);
}
