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
    if !s.followed_artists.is_empty() {
      s.followed_artists_index =
        (s.followed_artists_index + step).min(s.followed_artists.len() - 1);
    }
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    let mut s = state.lock().unwrap();
    s.followed_artists_index = s.followed_artists_index.saturating_sub(step);
    return;
  }
  if keys.activate.matches(&key) {
    let info = {
      let s = state.lock().unwrap();
      s.followed_artists
        .get(s.followed_artists_index)
        .map(|a| (a.id.id().to_string(), a.name.clone()))
    };
    if let Some((artist_id, artist_name)) = info {
      let _ = io_tx
        .send(IoEvent::OpenArtist {
          artist_id,
          artist_name,
        })
        .await;
      state.lock().unwrap().push_block(ActiveBlock::ArtistView);
    }
  }
}
