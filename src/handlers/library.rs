use crate::app::{lock, ActiveBlock, AppState, LIBRARY_ENTRIES};
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
    let mut s = lock(state);
    s.library_index = (s.library_index + 1).min(LIBRARY_ENTRIES.len() - 1);
    return;
  }
  if keys.move_up.matches(&key) {
    let mut s = lock(state);
    s.library_index = s.library_index.saturating_sub(1);
    return;
  }
  if keys.activate.matches(&key) {
    let idx = lock(state).library_index;
    // Matched on `name`, not on the rendered label, so glyphs are free to
    // change without silently breaking navigation.
    match LIBRARY_ENTRIES.get(idx).map(|e| e.name) {
      Some("Liked Songs") => {
        super::send_io(state, io_tx, IoEvent::GetSavedTracks);
        lock(state).push_block(ActiveBlock::TrackTable);
      }
      Some("Albums") => {
        super::send_io(state, io_tx, IoEvent::GetSavedAlbums);
        lock(state).push_block(ActiveBlock::SavedAlbums);
      }
      Some("Artists") => {
        super::send_io(state, io_tx, IoEvent::GetFollowedArtists);
        lock(state).push_block(ActiveBlock::FollowedArtists);
      }
      Some("Recently Played") => {
        super::send_io(state, io_tx, IoEvent::GetRecentlyPlayed);
        lock(state).push_block(ActiveBlock::TrackTable);
      }
      Some("Podcasts") => {
        super::send_io(state, io_tx, IoEvent::GetSavedShows);
        lock(state).push_block(ActiveBlock::SavedShows);
      }
      _ => {}
    }
  }
}
