use crate::app::{ActiveBlock, AppState, ArtistTab};
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
  if keys.search_tab_prev.matches(&key) {
    let mut s = state.lock().unwrap();
    s.artist_view.tab = s.artist_view.tab.prev();
    return;
  }
  if keys.search_tab_next.matches(&key) {
    let mut s = state.lock().unwrap();
    s.artist_view.tab = s.artist_view.tab.next();
    return;
  }
  if keys.move_down.matches(&key) {
    move_selection(state, 1);
    return;
  }
  if keys.move_up.matches(&key) {
    move_selection(state, -1);
    return;
  }
  if keys.move_down_big.matches(&key) {
    move_selection(state, 5);
    return;
  }
  if keys.move_up_big.matches(&key) {
    move_selection(state, -5);
    return;
  }
  if keys.move_top.matches(&key) {
    set_index(state, 0);
    return;
  }
  if keys.move_bottom.matches(&key) {
    set_index(state, usize::MAX);
    return;
  }
  if keys.add_to_queue.matches(&key) {
    let uri = {
      let s = state.lock().unwrap();
      if matches!(s.artist_view.tab, ArtistTab::Tracks) {
        s.artist_view
          .tracks
          .get(s.artist_view.tracks_index)
          .and_then(|t| t.uri.clone())
      } else {
        None
      }
    };
    if let Some(uri) = uri {
      let _ = io_tx.send(IoEvent::AddToQueue(uri)).await;
    }
    return;
  }
  if keys.activate.matches(&key) {
    let action = {
      let s = state.lock().unwrap();
      match s.artist_view.tab {
        ArtistTab::Tracks => {
          let uris: Vec<String> = s
            .artist_view
            .tracks
            .iter()
            .filter_map(|t| t.uri.clone())
            .collect();
          if uris.is_empty() {
            None
          } else {
            let idx = s.artist_view.tracks_index.min(uris.len() - 1);
            Some(Action::Play(uris, idx))
          }
        }
        ArtistTab::Albums => s
          .artist_view
          .albums
          .get(s.artist_view.albums_index)
          .and_then(|a| {
            a.id
              .as_ref()
              .map(|id| Action::OpenAlbum(id.id().to_string(), a.name.clone()))
          }),
      }
    };
    match action {
      Some(Action::Play(uris, idx)) => {
        let _ = io_tx
          .send(IoEvent::PlayTrackUris {
            uris,
            offset_index: idx,
          })
          .await;
      }
      Some(Action::OpenAlbum(id, name)) => {
        let _ = io_tx
          .send(IoEvent::GetAlbumTracks {
            album_id: id,
            album_name: name,
          })
          .await;
        state.lock().unwrap().push_block(ActiveBlock::TrackTable);
      }
      None => {}
    }
  }
}

enum Action {
  Play(Vec<String>, usize),
  OpenAlbum(String, String),
}

fn move_selection(state: &Mutex<AppState>, delta: i32) {
  let mut s = state.lock().unwrap();
  let (idx, max_len) = match s.artist_view.tab {
    ArtistTab::Tracks => (
      s.artist_view.tracks_index,
      s.artist_view.tracks.len(),
    ),
    ArtistTab::Albums => (
      s.artist_view.albums_index,
      s.artist_view.albums.len(),
    ),
  };
  if max_len == 0 {
    return;
  }
  let new = (idx as i32 + delta).clamp(0, max_len as i32 - 1) as usize;
  match s.artist_view.tab {
    ArtistTab::Tracks => s.artist_view.tracks_index = new,
    ArtistTab::Albums => s.artist_view.albums_index = new,
  }
}

fn set_index(state: &Mutex<AppState>, target: usize) {
  let mut s = state.lock().unwrap();
  match s.artist_view.tab {
    ArtistTab::Tracks => {
      let max = s.artist_view.tracks.len().saturating_sub(1);
      s.artist_view.tracks_index = target.min(max);
    }
    ArtistTab::Albums => {
      let max = s.artist_view.albums.len().saturating_sub(1);
      s.artist_view.albums_index = target.min(max);
    }
  }
}
