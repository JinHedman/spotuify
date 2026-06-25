use crate::app::AppState;
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crossterm::event::KeyEvent;
use rspotify::prelude::Id;
use std::sync::Mutex;
use tokio::sync::mpsc;

pub(super) async fn handle(
  key: KeyEvent,
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
  keys: &KeyBindings,
) {
  if keys.move_down.matches(&key) || keys.move_down_big.matches(&key) {
    let step = if keys.move_down_big.matches(&key) {
      5
    } else {
      1
    };
    let mut s = state.lock().unwrap();
    if !s.show_episodes.is_empty() {
      s.show_episodes_index = (s.show_episodes_index + step).min(s.show_episodes.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    let mut s = state.lock().unwrap();
    s.show_episodes_index = s.show_episodes_index.saturating_sub(step);
    return;
  }
  if keys.add_to_queue.matches(&key) {
    let uri = {
      let s = state.lock().unwrap();
      s.show_episodes
        .get(s.show_episodes_index)
        .map(|e| e.id.uri())
    };
    if let Some(uri) = uri {
      let _ = io_tx.send(IoEvent::AddToQueue(uri)).await;
    }
    return;
  }
  if keys.activate.matches(&key) {
    let uri = {
      let s = state.lock().unwrap();
      s.show_episodes
        .get(s.show_episodes_index)
        .map(|e| e.id.uri())
    };
    if let Some(uri) = uri {
      let _ = io_tx.send(IoEvent::PlayUri(uri)).await;
    }
  }
}
