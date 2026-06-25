use crate::app::AppState;
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crate::config::presets::PRESETS;
use crate::config::selected_theme_path;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::Mutex;
use tokio::sync::mpsc;
use tracing::warn;

pub(super) async fn handle(
  key: KeyEvent,
  state: &Mutex<AppState>,
  _io_tx: &mpsc::Sender<IoEvent>,
  keys: &KeyBindings,
) {
  // Cancel: revert to the saved theme and close.
  if keys.quit.matches(&key) || keys.back.matches(&key) {
    let mut s = state.lock().unwrap();
    if let Some(saved) = s.theme_before_preview.take() {
      s.theme = saved;
    }
    s.pop_block();
    return;
  }

  // Commit: keep whatever theme is currently previewed, persist the choice,
  // and close. Persistence is best-effort — if the write fails we log and
  // continue; the in-session theme still stays.
  if matches!(key.code, KeyCode::Enter) {
    let name = {
      let mut s = state.lock().unwrap();
      s.theme_before_preview = None;
      s.pop_block();
      PRESETS.get(s.theme_picker_index).map(|p| p.name)
    };
    if let Some(name) = name {
      if let Err(err) = persist_selected_theme(name) {
        warn!(?err, preset = %name, "failed to persist selected theme");
      }
    }
    return;
  }

  if keys.move_down.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = (s.theme_picker_index + 1).min(PRESETS.len().saturating_sub(1));
    s.theme = PRESETS[s.theme_picker_index].theme();
    return;
  }
  if keys.move_up.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = s.theme_picker_index.saturating_sub(1);
    s.theme = PRESETS[s.theme_picker_index].theme();
    return;
  }
  if keys.move_top.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = 0;
    s.theme = PRESETS[s.theme_picker_index].theme();
    return;
  }
  if keys.move_bottom.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = PRESETS.len().saturating_sub(1);
    s.theme = PRESETS[s.theme_picker_index].theme();
  }
}

fn persist_selected_theme(name: &str) -> anyhow::Result<()> {
  let path = selected_theme_path()?;
  std::fs::write(&path, name)?;
  Ok(())
}
