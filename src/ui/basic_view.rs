use crate::app::AppState;
use crate::ui::playbar;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  Frame,
};

/// Minimal layout for short terminals: playbar centered vertically, nothing else.
/// Used when `area.height < BASIC_VIEW_HEIGHT` (but above the MIN floor).
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  // Sized from the shared constant rather than a literal: the playbar grew to
  // three content rows, and a hardcoded 4 here silently clipped its progress
  // bar in short terminals.
  let bar_height = super::PLAYBAR_HEIGHT.min(area.height);
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
