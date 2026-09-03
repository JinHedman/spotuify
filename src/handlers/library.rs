use crate::app::{ActiveBlock, AppState, LIBRARY_ENTRIES};
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
  if keys.move_down.matches(&key) {
    let mut s = state.lock().unwrap();
    s.library_index = (s.library_index + 1).min(LIBRARY_ENTRIES.len() - 1);
    return;
  }
  if keys.move_up.matches(&key) {
    let mut s = state.lock().unwrap();
    s.library_index = s.library_index.saturating_sub(1);
    return;
  }
  if keys.activate.matches(&key) {
    let idx = state.lock().unwrap().library_index;
    // Matched on `name`, not on the rendered label, so glyphs are free to
    // change without silently breaking navigation.
    match LIBRARY_ENTRIES.get(idx).map(|e| e.name) {
      Some("Liked Songs") => {
        let _ = io_tx.send(IoEvent::GetSavedTracks).await;
        state.lock().unwrap().push_block(ActiveBlock::TrackTable);
      }
      Some("Albums") => {
        let _ = io_tx.send(IoEvent::GetSavedAlbums).await;
        state.lock().unwrap().push_block(ActiveBlock::SavedAlbums);
      }
      Some("Artists") => {
        let _ = io_tx.send(IoEvent::GetFollowedArtists).await;
        state
          .lock()
          .unwrap()
          .push_block(ActiveBlock::FollowedArtists);
      }
      Some("Recently Played") => {
        let _ = io_tx.send(IoEvent::GetRecentlyPlayed).await;
        state.lock().unwrap().push_block(ActiveBlock::TrackTable);
      }
      Some("Podcasts") => {
        let _ = io_tx.send(IoEvent::GetSavedShows).await;
        state.lock().unwrap().push_block(ActiveBlock::SavedShows);
      }
      _ => {}
    }
  }
}
