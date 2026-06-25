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
  if matches!(key.code, KeyCode::Esc)
    || keys.quit.matches(&key)
    || keys.queue.matches(&key)
    || keys.back.matches(&key)
  {
    state.lock().unwrap().pop_block();
    return;
  }
  if keys.move_down.matches(&key) || keys.move_down_big.matches(&key) {
    let step = if keys.move_down_big.matches(&key) {
      5
    } else {
      1
    };
    let mut s = state.lock().unwrap();
    if !s.queue_items.is_empty() {
      s.queue_index = (s.queue_index + step).min(s.queue_items.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    let mut s = state.lock().unwrap();
    s.queue_index = s.queue_index.saturating_sub(step);
    return;
  }
  if keys.refresh.matches(&key) {
    let _ = io_tx.send(IoEvent::GetQueue).await;
  }
}
