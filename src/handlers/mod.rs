mod artist_view;
mod dialog;
mod followed_artists;
mod input;
mod library;
mod playlists;
mod queue;
mod saved_albums;
mod saved_shows;
mod search_results;
mod select_device;
mod show_episodes;
mod theme_picker;
mod track_table;

use crate::app::{ActiveBlock, AppState};
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crate::config::user::UserConfig;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub enum KeyOutcome {
  Continue,
  Quit,
}

pub async fn handle_key(
  key: KeyEvent,
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
) -> KeyOutcome {
  let config: Arc<UserConfig> = state.lock().unwrap().config.clone();
  let keys: &KeyBindings = &config.keys;
  let behavior = &config.behavior;

  // Ctrl+C always quits — hard-wired, not user-configurable.
  if matches!(
    (key.code, key.modifiers),
    (KeyCode::Char('c'), KeyModifiers::CONTROL)
  ) {
    return KeyOutcome::Quit;
  }

  // Overlays get first crack.
  if state.lock().unwrap().help_visible {
    if keys.help.matches(&key) || keys.quit.matches(&key) {
      state.lock().unwrap().help_visible = false;
    }
    return KeyOutcome::Continue;
  }

  let active = state.lock().unwrap().active_block;

  if active == ActiveBlock::Dialog {
    dialog::handle(key, state, io_tx, keys).await;
    return KeyOutcome::Continue;
  }

  if active == ActiveBlock::ThemePicker {
    theme_picker::handle(key, state, io_tx, keys).await;
    return KeyOutcome::Continue;
  }

  if active == ActiveBlock::SearchInput {
    input::handle(key, state, io_tx, keys).await;
    return KeyOutcome::Continue;
  }

  if active == ActiveBlock::SelectDevice {
    select_device::handle(key, state, io_tx, keys).await;
    return KeyOutcome::Continue;
  }

  if active == ActiveBlock::Queue {
    queue::handle(key, state, io_tx, keys).await;
    return KeyOutcome::Continue;
  }

  if keys.quit.matches(&key) {
    return KeyOutcome::Quit;
  }
  if keys.help.matches(&key) {
    state.lock().unwrap().help_visible = true;
    return KeyOutcome::Continue;
  }
  if keys.search.matches(&key) {
    state.lock().unwrap().push_block(ActiveBlock::SearchInput);
    return KeyOutcome::Continue;
  }
  if keys.device.matches(&key) {
    let _ = io_tx.send(IoEvent::GetDevices).await;
    state.lock().unwrap().push_block(ActiveBlock::SelectDevice);
    return KeyOutcome::Continue;
  }
  if keys.queue.matches(&key) {
    let _ = io_tx.send(IoEvent::GetQueue).await;
    state.lock().unwrap().push_block(ActiveBlock::Queue);
    return KeyOutcome::Continue;
  }
  if keys.play_pause.matches(&key) {
    let is_playing = state.lock().unwrap().is_playing();
    let ev = if is_playing {
      IoEvent::PausePlayback
    } else {
      IoEvent::ResumePlayback
    };
    let _ = io_tx.send(ev).await;
    return KeyOutcome::Continue;
  }
  if keys.next_track.matches(&key) {
    let _ = io_tx.send(IoEvent::NextTrack).await;
    return KeyOutcome::Continue;
  }
  if keys.previous_track.matches(&key) {
    let _ = io_tx.send(IoEvent::PreviousTrack).await;
    return KeyOutcome::Continue;
  }
  if keys.volume_up.matches(&key) {
    let v = state.lock().unwrap().current_volume();
    let _ = io_tx
      .send(IoEvent::ChangeVolume(
        v.saturating_add(behavior.volume_step).min(100),
      ))
      .await;
    return KeyOutcome::Continue;
  }
  if keys.volume_down.matches(&key) {
    let v = state.lock().unwrap().current_volume();
    let _ = io_tx
      .send(IoEvent::ChangeVolume(
        v.saturating_sub(behavior.volume_step),
      ))
      .await;
    return KeyOutcome::Continue;
  }
  if keys.seek_backward.matches(&key) {
    let progress = state.lock().unwrap().current_progress_ms();
    if let Some(p) = progress {
      let _ = io_tx
        .send(IoEvent::Seek((p - behavior.seek_step_ms).max(0)))
        .await;
    }
    return KeyOutcome::Continue;
  }
  if keys.seek_forward.matches(&key) {
    let progress = state.lock().unwrap().current_progress_ms();
    if let Some(p) = progress {
      let _ = io_tx.send(IoEvent::Seek(p + behavior.seek_step_ms)).await;
    }
    return KeyOutcome::Continue;
  }
  if keys.shuffle.matches(&key) {
    let _ = io_tx.send(IoEvent::ToggleShuffle).await;
    return KeyOutcome::Continue;
  }
  if keys.repeat.matches(&key) {
    let _ = io_tx.send(IoEvent::CycleRepeat).await;
    return KeyOutcome::Continue;
  }
  if keys.refresh.matches(&key) {
    let _ = io_tx.send(IoEvent::GetCurrentPlayback).await;
    return KeyOutcome::Continue;
  }
  if keys.save_track.matches(&key) {
    let track_id = state.lock().unwrap().current_track_id();
    if let Some(id) = track_id {
      let _ = io_tx.send(IoEvent::ToggleSaveTrack(id)).await;
    }
    return KeyOutcome::Continue;
  }
  if keys.save_album.matches(&key) {
    let album_id = state.lock().unwrap().current_album_id();
    if let Some(id) = album_id {
      let _ = io_tx.send(IoEvent::ToggleSaveAlbum(id)).await;
    }
    return KeyOutcome::Continue;
  }
  if keys.follow_artist.matches(&key) {
    let artist_id = state.lock().unwrap().current_artist_id();
    if let Some(id) = artist_id {
      let _ = io_tx.send(IoEvent::ToggleFollowArtist(id)).await;
    }
    return KeyOutcome::Continue;
  }
  if keys.theme_picker.matches(&key) {
    let mut s = state.lock().unwrap();
    s.theme_before_preview = Some(s.theme);
    // Default the cursor to whichever preset matches the current theme, so
    // the cancel/revert path is a no-op for users already on a preset.
    s.theme_picker_index = crate::config::presets::PRESETS
      .iter()
      .position(|p| p.theme() == s.theme)
      .unwrap_or(0);
    s.push_block(ActiveBlock::ThemePicker);
    return KeyOutcome::Continue;
  }
  if keys.block_left.matches(&key) {
    let mut s = state.lock().unwrap();
    s.active_block = s.active_block.go_left();
    return KeyOutcome::Continue;
  }
  if keys.block_right.matches(&key) {
    let mut s = state.lock().unwrap();
    s.active_block = s.active_block.go_right();
    return KeyOutcome::Continue;
  }
  if keys.block_up.matches(&key) {
    let mut s = state.lock().unwrap();
    s.active_block = s.active_block.go_up();
    return KeyOutcome::Continue;
  }
  if keys.block_down.matches(&key) {
    let mut s = state.lock().unwrap();
    s.active_block = s.active_block.go_down();
    return KeyOutcome::Continue;
  }
  if keys.back.matches(&key) {
    let mut s = state.lock().unwrap();
    if !s.pop_block() && !s.active_block.is_home() {
      s.active_block = ActiveBlock::Library;
    }
    return KeyOutcome::Continue;
  }

  match active {
    ActiveBlock::Library => library::handle(key, state, io_tx, keys).await,
    ActiveBlock::MyPlaylists => playlists::handle(key, state, io_tx, keys).await,
    ActiveBlock::TrackTable => track_table::handle(key, state, io_tx, keys).await,
    ActiveBlock::SearchResults => search_results::handle(key, state, io_tx, keys).await,
    ActiveBlock::SavedAlbums => saved_albums::handle(key, state, io_tx, keys).await,
    ActiveBlock::FollowedArtists => followed_artists::handle(key, state, io_tx, keys).await,
    ActiveBlock::ArtistView => artist_view::handle(key, state, io_tx, keys).await,
    ActiveBlock::SavedShows => saved_shows::handle(key, state, io_tx, keys).await,
    ActiveBlock::ShowEpisodes => show_episodes::handle(key, state, io_tx, keys).await,
    ActiveBlock::SearchInput
    | ActiveBlock::SelectDevice
    | ActiveBlock::Queue
    | ActiveBlock::Dialog
    | ActiveBlock::ThemePicker => {}
  }

  KeyOutcome::Continue
}
