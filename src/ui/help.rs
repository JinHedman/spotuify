use crate::app::AppState;
use ratatui::{
  layout::{Constraint, Direction, Layout, Rect},
  style::{Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, Paragraph},
  Frame,
};

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
  let rows = help_lines(state);
  let wanted = rows.len() as u16 + 2;
  let height = wanted
    .min(MAX_HEIGHT)
    .min(area.height.saturating_sub(MARGIN));
  let popup = centered(area, 60, height);
  frame.render_widget(Clear, popup);

  let visible = popup.height.saturating_sub(2);
  let max_scroll = (rows.len() as u16).saturating_sub(visible);
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
      (scroll + visible).min(rows.len() as u16),
      rows.len()
    )
  };

  let block = Block::new()
    .borders(Borders::ALL)
    .title(title)
    .border_style(Style::default().fg(theme.active));
  let inner = block.inner(popup);
  frame.render_widget(block, popup);

  let lines = rows;

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

/// The overlay's contents, generated from the live keybindings.
///
/// Previously a hardcoded array of key strings, which meant it stated the
/// defaults rather than the user's actual bindings — it still said `z` for
/// shuffle after a rebind. Section headers come from the same table, so a new
/// action appears here by being added once in `KeyBindings::all`.
fn help_lines(state: &AppState) -> Vec<Line<'static>> {
  let theme = state.theme;
  let mut lines: Vec<Line<'static>> = Vec::new();
  let mut current_section = "";

  for (section, label, keys) in state.config.keys.all() {
    if section != current_section {
      if !lines.is_empty() {
        lines.push(Line::raw(""));
      }
      lines.push(Line::from(Span::styled(
        section,
        Style::default()
          .fg(theme.active)
          .add_modifier(Modifier::BOLD),
      )));
      current_section = section;
    }
    lines.push(Line::from(vec![
      Span::styled(
        format!("  {:<22}", keys.describe()),
        Style::default().add_modifier(Modifier::BOLD),
      ),
      Span::styled(label, Style::default().fg(theme.hint)),
    ]));
  }

  lines.push(Line::raw(""));
  lines.push(Line::from(Span::styled(
    "Ctrl+C always quits, whatever `quit` is bound to.",
    Style::default().fg(theme.hint),
  )));
  lines
}

#[cfg(test)]
mod tests {
  use super::help_lines;
  use crate::app::AppState;
  use crate::config::keys::{KeyBindings, KeyList};
  use crate::config::user::UserConfig;
  use std::sync::Arc;

  fn state_with(keys: KeyBindings) -> AppState {
    let mut cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    cfg.keys = keys;
    AppState::new(Arc::new(cfg))
  }

  fn text(state: &AppState) -> String {
    help_lines(state)
      .iter()
      .map(|l| {
        l.spans
          .iter()
          .map(|s| s.content.as_ref())
          .collect::<String>()
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// The bug this replaces: the overlay held hardcoded key strings, so it
  /// advertised the defaults no matter what the user had bound.
  #[test]
  fn the_overlay_shows_the_users_bindings_not_the_defaults() {
    // Struct update rather than assign-after-default: clippy's
    // field_reassign_with_default rejects the latter.
    let keys = KeyBindings {
      shuffle: KeyList::single("ctrl+z"),
      ..KeyBindings::default()
    };
    let s = state_with(keys);
    let out = text(&s);

    assert!(
      out.contains("ctrl+z"),
      "rebound key must appear in the help:\n{out}"
    );
    // The default binding must be gone, not merely joined by the new one.
    let shuffle_line = out
      .lines()
      .find(|l| l.contains("toggle shuffle"))
      .expect("shuffle row present");
    assert!(
      !shuffle_line.contains(" z "),
      "still advertising the default: {shuffle_line}"
    );
  }

  #[test]
  fn every_action_is_listed_under_a_section() {
    let s = state_with(KeyBindings::default());
    let out = text(&s);
    for section in ["Navigation", "Search", "Playback", "Library", "Help"] {
      assert!(out.contains(section), "missing section {section}:\n{out}");
    }
    for label in ["toggle shuffle", "cycle repeat", "select device", "quit"] {
      assert!(out.contains(label), "missing action {label}:\n{out}");
    }
  }

  /// Alternatives are shown together, so `back` reads as all four keys.
  #[test]
  fn alternatives_are_listed_together() {
    let s = state_with(KeyBindings::default());
    let out = text(&s);
    let back = out
      .lines()
      .find(|l| l.ends_with("back"))
      .expect("back row present");
    assert!(back.contains('/'), "alternatives joined: {back}");
  }

  /// Ctrl+C is hard-wired and not in the keymap, so it needs saying.
  #[test]
  fn the_hardwired_quit_is_mentioned() {
    let s = state_with(KeyBindings::default());
    assert!(text(&s).contains("Ctrl+C"));
  }
}
