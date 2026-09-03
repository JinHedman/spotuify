use crate::app::AppState;
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crate::config::presets::{PresetKind, PRESETS};
use crate::config::{selected_theme_path, time_of_day_path};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::Mutex;
use std::time::Duration;
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
    s.cancel_theme_preview();
    s.pop_block();
    return;
  }

  // Space toggles the after-dark modifier when the cursor is on its row.
  //
  // Applied and persisted immediately rather than folded into the preview
  // session, because it is a modifier and not one of the alternatives: Esc
  // reverts the theme you were auditioning, and does not undo a switch you
  // deliberately flipped. The hint line says so.
  if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
    // Enter is included so the toggle row cannot be "committed" like a theme:
    // doing so would persist its name into .selected_theme, and restoring it
    // next launch would select an entry that is not a theme at all.
    let toggled = {
      let mut s = state.lock().unwrap();
      let on_toggle = PRESETS
        .get(s.theme_picker_index)
        .is_some_and(|p| p.kind == PresetKind::AfterDark);
      if on_toggle {
        let strength = s.toggle_after_dark();
        let ms = s.config.behavior.theme_transition_ms;
        s.apply_theme_source(Duration::from_millis(ms));
        Some(strength)
      } else {
        None
      }
    };
    if let Some(strength) = toggled {
      if let Err(err) = persist_time_of_day(strength) {
        warn!(?err, "failed to persist after-dark setting");
      }
      return;
    }
    // Any other row: fall through to the commit below.
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
    let (index, ms) = (s.theme_picker_index, s.config.behavior.theme_transition_ms);
    s.select_preset(index, Duration::from_millis(ms));
    return;
  }
  if keys.move_up.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = s.theme_picker_index.saturating_sub(1);
    let (index, ms) = (s.theme_picker_index, s.config.behavior.theme_transition_ms);
    s.select_preset(index, Duration::from_millis(ms));
    return;
  }
  if keys.move_top.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = 0;
    let (index, ms) = (s.theme_picker_index, s.config.behavior.theme_transition_ms);
    s.select_preset(index, Duration::from_millis(ms));
    return;
  }
  if keys.move_bottom.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_picker_index = PRESETS.len().saturating_sub(1);
    let (index, ms) = (s.theme_picker_index, s.config.behavior.theme_transition_ms);
    s.select_preset(index, Duration::from_millis(ms));
  }
}

fn persist_time_of_day(strength: f32) -> anyhow::Result<()> {
  std::fs::write(time_of_day_path()?, strength.to_string())?;
  Ok(())
}

fn persist_selected_theme(name: &str) -> anyhow::Result<()> {
  let path = selected_theme_path()?;
  std::fs::write(&path, name)?;
  Ok(())
}
