use crate::app::ActiveBlock;
use crate::config::theme::Theme;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};

pub fn block<'a>(
  title: &'a str,
  block_id: ActiveBlock,
  active: ActiveBlock,
  theme: &Theme,
) -> Block<'a> {
  let color = if block_id == active {
    theme.active
  } else {
    theme.inactive
  };
  Block::new()
    .borders(Borders::ALL)
    .title(format!(" {title} "))
    .border_style(Style::default().fg(color))
}
