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
    if !s.saved_albums.is_empty() {
      s.saved_albums_index = (s.saved_albums_index + step).min(s.saved_albums.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    let mut s = state.lock().unwrap();
    s.saved_albums_index = s.saved_albums_index.saturating_sub(step);
    return;
  }
  if keys.activate.matches(&key) {
    let info = {
      let s = state.lock().unwrap();
      s.saved_albums
        .get(s.saved_albums_index)
        .map(|sa| (sa.album.id.id().to_string(), sa.album.name.clone()))
    };
    if let Some((album_id, album_name)) = info {
      let _ = io_tx
        .send(IoEvent::GetAlbumTracks {
          album_id,
          album_name,
        })
        .await;
      state.lock().unwrap().push_block(ActiveBlock::TrackTable);
    }
  }
}
