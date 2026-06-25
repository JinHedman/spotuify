use crate::app::{ActiveBlock, AppState};
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
    if !s.saved_shows.is_empty() {
      s.saved_shows_index = (s.saved_shows_index + step).min(s.saved_shows.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    let mut s = state.lock().unwrap();
    s.saved_shows_index = s.saved_shows_index.saturating_sub(step);
    return;
  }
  if keys.activate.matches(&key) {
    let info = {
      let s = state.lock().unwrap();
      s.saved_shows
        .get(s.saved_shows_index)
        .map(|sh| (sh.show.id.id().to_string(), sh.show.name.clone()))
    };
    if let Some((show_id, show_name)) = info {
      let _ = io_tx
        .send(IoEvent::GetShowEpisodes { show_id, show_name })
        .await;
      state.lock().unwrap().push_block(ActiveBlock::ShowEpisodes);
    }
  }
}
