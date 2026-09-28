use crate::app::{ActiveBlock, AppState, Dialog, DialogAction};
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
    {
      let mut s = state.lock().unwrap();
      if !s.playlists.is_empty() {
        s.playlists_index = (s.playlists_index + step).min(s.playlists.len() - 1);
      }
    }
    send_cover_refresh(state, io_tx);
    return;
  }
  if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
    let step = if keys.move_up_big.matches(&key) { 5 } else { 1 };
    {
      let mut s = state.lock().unwrap();
      s.playlists_index = s.playlists_index.saturating_sub(step);
    }
    send_cover_refresh(state, io_tx);
    return;
  }
  if keys.delete_playlist.matches(&key) {
    let mut s = state.lock().unwrap();
    if let Some(p) = s.playlists.get(s.playlists_index) {
      let playlist_id = p.id.id().to_string();
      let name = p.name.clone();
      s.dialog = Some(Dialog {
        message: format!("Remove “{name}” from your library?"),
        action: DialogAction::UnfollowPlaylist { playlist_id },
      });
      s.push_block(ActiveBlock::Dialog);
    }
    return;
  }
  if keys.activate.matches(&key) {
    let info = {
      let s = state.lock().unwrap();
      s.playlists
        .get(s.playlists_index)
        .map(|p| (p.id.id().to_string(), p.name.clone()))
    };
    if let Some((playlist_id, playlist_name)) = info {
      super::send_io(
        state,
        io_tx,
        IoEvent::GetPlaylistTracks {
          playlist_id,
          playlist_name,
        },
      );
      state.lock().unwrap().active_block = ActiveBlock::TrackTable;
    }
  }
}

/// Coalesced and non-blocking: holding j/k while the network task is stuck
/// would otherwise fill the channel and freeze the UI on the next send.
fn send_cover_refresh(state: &Mutex<AppState>, io_tx: &mpsc::Sender<IoEvent>) {
  let pending = state.lock().unwrap().pending_io.clone();
  pending.send(io_tx, IoEvent::RefreshPlaylistCover);
}
