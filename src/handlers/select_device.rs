use crate::app::AppState;
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::Mutex;
use tokio::sync::mpsc;

pub(super) async fn handle(
  key: KeyEvent,
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
  keys: &KeyBindings,
) {
  // Close on Esc or configured quit/back keys.
  if matches!(key.code, KeyCode::Esc) || keys.quit.matches(&key) || keys.back.matches(&key) {
    state.lock().unwrap().pop_block();
    return;
  }
  if keys.move_down.matches(&key) {
    let mut s = state.lock().unwrap();
    if !s.devices.is_empty() {
      s.devices_index = (s.devices_index + 1).min(s.devices.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) {
    let mut s = state.lock().unwrap();
    s.devices_index = s.devices_index.saturating_sub(1);
    return;
  }
  if keys.refresh.matches(&key) {
    let _ = io_tx.send(IoEvent::GetDevices).await;
    return;
  }
  if keys.activate.matches(&key) {
    let device_id = {
      let s = state.lock().unwrap();
      s.devices.get(s.devices_index).and_then(|d| d.id.clone())
    };
    if let Some(id) = device_id {
      let _ = io_tx.send(IoEvent::TransferPlayback(id)).await;
      state.lock().unwrap().pop_block();
    }
  }
}
