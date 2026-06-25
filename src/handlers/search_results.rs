use crate::app::{ActiveBlock, AppState, SearchTab};
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crate::ui::search_results as results_helpers;
use crossterm::event::KeyEvent;
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
    s.search_tab = s.search_tab.prev();
    return;
  }
  if keys.search_tab_next.matches(&key) {
    let mut s = state.lock().unwrap();
    s.search_tab = s.search_tab.next();
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
  if keys.add_to_queue.matches(&key) {
    let uri = {
      let s = state.lock().unwrap();
      if s.search_tab == SearchTab::Tracks {
        results_helpers::selected_track_uri(&s)
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
    let ev = {
      let s = state.lock().unwrap();
      pick_event(&s)
    };
    match ev {
      Some(PickEvent::PlaySingleTrack(_uri, uris, idx)) => {
        let _ = io_tx
          .send(IoEvent::PlayTrackUris {
            uris,
            offset_index: idx,
          })
          .await;
      }
      Some(PickEvent::OpenAlbum(id, name)) => {
        let _ = io_tx
          .send(IoEvent::GetAlbumTracks {
            album_id: id,
            album_name: name,
          })
          .await;
        state.lock().unwrap().push_block(ActiveBlock::TrackTable);
      }
      Some(PickEvent::OpenArtist(id, name)) => {
        let _ = io_tx
          .send(IoEvent::OpenArtist {
            artist_id: id,
            artist_name: name,
          })
          .await;
        state.lock().unwrap().push_block(ActiveBlock::ArtistView);
      }
      None => {}
    }
  }
}

enum PickEvent {
  PlaySingleTrack(Option<String>, Vec<String>, usize),
  OpenAlbum(String, String),
  OpenArtist(String, String),
}

fn pick_event(state: &AppState) -> Option<PickEvent> {
  match state.search_tab {
    SearchTab::Tracks => {
      let uris = results_helpers::all_track_uris(state);
      if uris.is_empty() {
        return None;
      }
      let idx = state.search_results.tracks_index.min(uris.len() - 1);
      Some(PickEvent::PlaySingleTrack(
        results_helpers::selected_track_uri(state),
        uris,
        idx,
      ))
    }
    SearchTab::Albums => {
      let (id, name) = results_helpers::selected_album(state)?;
      Some(PickEvent::OpenAlbum(id, name))
    }
    SearchTab::Artists => {
      let (id, name) = results_helpers::selected_artist(state)?;
      Some(PickEvent::OpenArtist(id, name))
    }
  }
}

fn move_selection(state: &Mutex<AppState>, delta: i32) {
  let mut s = state.lock().unwrap();
  let max_len = match s.search_tab {
    SearchTab::Tracks => s.search_results.tracks.len(),
    SearchTab::Albums => s.search_results.albums.len(),
    SearchTab::Artists => s.search_results.artists.len(),
  };
  if max_len == 0 {
    return;
  }
  let idx_mut = match s.search_tab {
    SearchTab::Tracks => &mut s.search_results.tracks_index,
    SearchTab::Albums => &mut s.search_results.albums_index,
    SearchTab::Artists => &mut s.search_results.artists_index,
  };
  let cur = *idx_mut as i32 + delta;
  *idx_mut = cur.clamp(0, max_len as i32 - 1) as usize;
}
