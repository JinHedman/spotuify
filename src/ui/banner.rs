use crate::config::theme::Theme;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::Paragraph,
  Frame,
};

const BANNER: &str = r"   _____             __        _
  / ___/____  ____  / /___  __(_)
  \__ \/ __ \/ __ \/ __/ / / / /
 ___/ / /_/ / /_/ / /_/ /_/ / /
/____/ .___/\____/\__/\__,_/_/
    /_/";

/// Visual width of the art itself (widest line). The *column* the banner is
/// rendered into can be wider — `draw` horizontally centers the art inside
/// whatever `area` it's given.
pub const BANNER_VISUAL_WIDTH: u16 = 33;
pub const BANNER_HEIGHT: u16 = 6;

pub fn draw(frame: &mut Frame, area: Rect, theme: &Theme) {
  // Pick accent colors from the theme so the banner tracks whatever palette
  // the user is on. active/progress/playing_icon are the three "accent" slots
  // every preset defines; cycling through them gives a subtle gradient on
  // themes where they differ and a clean monochrome on themes where they don't.
  let accents = [theme.active, theme.progress, theme.playing_icon];

  let lines: Vec<Line> = BANNER
    .lines()
    .enumerate()
    .map(|(i, l)| {
      let color = accents[(i / 2) % accents.len()];
      Line::from(Span::styled(
        l.to_string(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
      ))
    })
    .collect();

  // Horizontally center the art inside `area` by carving out a sub-rect of
  // `BANNER_VISUAL_WIDTH`. Rendering each line with Paragraph's center-alignment
  // would mis-shear the ASCII art (each line has a different length), so we
  // pad on the layout level instead.
  let art_width = BANNER_VISUAL_WIDTH.min(area.width);
  let art_area = Layout::new(
    Direction::Horizontal,
    [
      Constraint::Length(area.width.saturating_sub(art_width) / 2),
      Constraint::Length(art_width),
      Constraint::Min(0),
    ],
  )
  .split(area)[1];

  frame.render_widget(Paragraph::new(lines), art_area);
}
