use crate::app::AppState;
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crossterm::event::KeyEvent;
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
    if !s.track_list.is_empty() {
      s.track_list_index = (s.track_list_index + step).min(s.track_list.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    let mut s = state.lock().unwrap();
    s.track_list_index = s.track_list_index.saturating_sub(step);
    return;
  }
  if keys.move_top.matches(&key) {
    state.lock().unwrap().track_list_index = 0;
    return;
  }
  if keys.move_bottom.matches(&key) {
    let mut s = state.lock().unwrap();
    if !s.track_list.is_empty() {
      s.track_list_index = s.track_list.len() - 1;
    }
    return;
  }
  if keys.add_to_queue.matches(&key) {
    let uri = {
      let s = state.lock().unwrap();
      s.track_list
        .get(s.track_list_index)
        .and_then(|t| t.uri.clone())
    };
    if let Some(uri) = uri {
      let _ = io_tx.send(IoEvent::AddToQueue(uri)).await;
    }
    return;
  }
  if keys.activate.matches(&key) {
    let play = {
      let s = state.lock().unwrap();
      let idx = s.track_list_index;
      if s.track_list.is_empty() {
        None
      } else if let Some(ctx) = &s.track_list_context_uri {
        let track_uri = s.track_list.get(idx).and_then(|t| t.uri.clone());
        Some(IoEvent::PlayTrackInContext {
          context_uri: ctx.clone(),
          offset_index: idx,
          track_uri,
        })
      } else {
        let uris: Vec<String> = s.track_list.iter().filter_map(|t| t.uri.clone()).collect();
        Some(IoEvent::PlayTrackUris {
          uris,
          offset_index: idx,
        })
      }
    };
    if let Some(ev) = play {
      let _ = io_tx.send(ev).await;
    }
  }
}
