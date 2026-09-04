use crate::app::AppState;
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

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = &state.theme;

  // A message takes over this row for its lifetime. It used to replace the
  // whole playbar, which hid the track, cover, progress and controls for
  // what is usually a transient and often unimportant failure. This is the
  // least valuable row on screen, which makes it the right one to borrow.
  if let Some(notice) = state.notice() {
    let line = Line::from(vec![
      Span::styled("! ", Style::default().fg(theme.error)),
      Span::styled(notice, Style::default().fg(theme.error)),
    ]);
    frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
    return;
  }

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

#[cfg(test)]
mod tests {
  use super::draw;
  use crate::app::AppState;
  use crate::config::user::UserConfig;
  use ratatui::{backend::TestBackend, Terminal};
  use std::sync::Arc;

  fn state() -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    AppState::new(Arc::new(cfg))
  }

  fn render(state: &AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 1)).unwrap();
    terminal.draw(|f| draw(f, f.area(), state)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.width)
      .map(|x| buf[(x, 0)].symbol().to_string())
      .collect()
  }

  #[test]
  fn the_keys_show_when_there_is_nothing_to_report() {
    let s = state();
    let out = render(&s);
    assert!(out.contains("help"), "legend renders: {out}");
    assert!(!out.contains('!'), "no notice marker: {out}");
  }

  #[test]
  fn a_notice_takes_over_the_row() {
    let mut s = state();
    s.set_notice("config.yml is invalid (line 3) — using defaults");
    let out = render(&s);
    assert!(out.contains("config.yml is invalid"), "notice shown: {out}");
    assert!(!out.contains("help"), "legend yields to it: {out}");
  }

  /// The whole point: a message must not cost the playbar. It used to return
  /// early there, hiding track, cover, progress and controls.
  #[test]
  fn a_notice_does_not_disturb_the_playbar() {
    let mut s = state();
    s.set_notice("something went wrong");
    let mut terminal = Terminal::new(TestBackend::new(60, 5)).unwrap();
    terminal
      .draw(|f| crate::ui::playbar::draw(f, f.area(), &s))
      .unwrap();
    let buf = terminal.backend().buffer().clone();
    let out: String = (0..buf.area.height)
      .map(|y| {
        (0..buf.area.width)
          .map(|x| buf[(x, y)].symbol().to_string())
          .collect::<String>()
      })
      .collect::<Vec<_>>()
      .join("\n");
    assert!(
      !out.contains("something went wrong"),
      "the message belongs on the status line, not here:\n{out}"
    );
    assert!(out.contains("Now Playing"), "playbar still drawn:\n{out}");
  }

  /// Notices expire on their own, so nothing has to own clearing them.
  #[test]
  fn a_notice_expires() {
    let mut s = state();
    s.set_notice("temporary");
    assert!(s.notice().is_some(), "visible immediately");

    // Reach past the lifetime without sleeping through it.
    if let Some(n) = s.last_error.as_mut() {
      n.expire_for_test();
    }
    assert!(s.notice().is_none(), "gone once its lifetime is up");
    let out = render(&s);
    assert!(out.contains("help"), "legend comes back: {out}");
  }
}
