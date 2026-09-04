use crate::app::AppState;
use ratatui::{
  layout::{Alignment, Rect},
  style::Style,
  text::{Line, Span},
  widgets::Paragraph,
  Frame,
};

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

  // Read from the live bindings by field, so a rebind is reflected here
  // instead of the row continuing to advertise the defaults. Referenced
  // directly rather than looked up by name: the compiler then catches a
  // renamed or removed action, which a string lookup would not.
  //
  // A subset, because the row is one line. The rest are in the help overlay.
  let k = &state.config.keys;
  let shown: [(&crate::config::keys::KeyList, &str); 9] = [
    (&k.help, "help"),
    (&k.search, "search"),
    (&k.device, "device"),
    (&k.queue, "queue"),
    (&k.save_track, "save"),
    (&k.theme_picker, "theme"),
    (&k.play_pause, "play/pause"),
    (&k.back, "back"),
    (&k.quit, "quit"),
  ];

  let mut spans: Vec<Span> = Vec::with_capacity(shown.len() * 4);
  for (keys, label) in shown {
    let key = keys.describe_first();
    if key.is_empty() {
      continue;
    }
    if !spans.is_empty() {
      spans.push(Span::styled("  ·  ", Style::default().fg(theme.hint)));
    }
    spans.push(Span::styled(key, Style::default().fg(theme.active)));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(label, Style::default().fg(theme.hint)));
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
