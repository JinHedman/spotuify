use crate::app::AppState;
use crate::ui::playbar;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  Frame,
};

/// Minimal layout for short terminals: playbar centered vertically, nothing else.
/// Used when `area.height < BASIC_VIEW_HEIGHT` (but above the MIN floor).
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  // Playbar needs 4 rows (title + 2 content + borders). Center it vertically
  // if we have a little more space to play with.
  let bar_height = 4u16.min(area.height);
  let top_pad = area.height.saturating_sub(bar_height) / 2;

  let rows = Layout::new(
    Direction::Vertical,
    [
      Constraint::Length(top_pad),
      Constraint::Length(bar_height),
      Constraint::Min(0),
    ],
  )
  .split(area);

  playbar::draw(frame, rows[1], state);
}
