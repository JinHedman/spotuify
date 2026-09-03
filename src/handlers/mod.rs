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
    let mut s = state.lock().unwrap();
    if keys.help.matches(&key) || keys.quit.matches(&key) || keys.back.matches(&key) {
      s.help_visible = false;
      return KeyOutcome::Continue;
    }
    // The list is longer than most terminals, so it scrolls with the same
    // keys as every other list. draw() clamps the upper bound, since only it
    // knows how many lines are visible.
    let step: u16 = if keys.move_down_big.matches(&key) || keys.move_up_big.matches(&key) {
      5
    } else {
      1
    };
    if keys.move_down.matches(&key) || keys.move_down_big.matches(&key) {
      s.help_scroll = s.help_scroll.saturating_add(step);
    } else if keys.move_up.matches(&key) || keys.move_up_big.matches(&key) {
      s.help_scroll = s.help_scroll.saturating_sub(step);
    } else if keys.move_top.matches(&key) {
      s.help_scroll = 0;
    } else if keys.move_bottom.matches(&key) {
      s.help_scroll = u16::MAX;
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
    let mut s = state.lock().unwrap();
    s.help_visible = true;
    // Always open at the top rather than wherever it was last left.
    s.help_scroll = 0;
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
    s.begin_theme_preview();
    // Default the cursor to whichever preset matches the current theme, so
    // the cancel/revert path is a no-op for users already on a preset.
    // Land on the entry matching the active source: the auto entry when it is
    // driving, otherwise whichever fixed palette is in use.
    use crate::config::presets::{PresetKind, PRESETS};
    let mode = s.theme_mode;
    let fixed = s.theme_fixed;
    s.theme_picker_index = PRESETS
      .iter()
      .position(|p| match mode {
        crate::app::ThemeMode::DecadeAuto => p.kind == PresetKind::DecadeAuto,
        crate::app::ThemeMode::EraAuto => p.kind == PresetKind::EraAuto,
        crate::app::ThemeMode::TimeOfDayAuto => p.kind == PresetKind::TimeOfDayAuto,
        crate::app::ThemeMode::Fixed => p.theme() == Some(fixed),
      })
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::user::UserConfig;
  use crossterm::event::KeyEventKind;

  fn test_state() -> Mutex<AppState> {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    Mutex::new(AppState::new(Arc::new(cfg)))
  }

  fn press(c: char) -> KeyEvent {
    KeyEvent {
      code: KeyCode::Char(c),
      modifiers: KeyModifiers::NONE,
      kind: KeyEventKind::Press,
      state: crossterm::event::KeyEventState::NONE,
    }
  }

  /// Also a deadlock guard: the overlay branch re-locks the state mutex inside
  /// a block whose condition also locked it. std::sync::Mutex is not
  /// reentrant, so if that temporary guard outlived the condition this test
  /// would hang instead of fail.
  #[tokio::test]
  async fn help_overlay_scrolls_and_closes() {
    let state = test_state();
    let (tx, _rx) = mpsc::channel::<IoEvent>(8);

    // Default bindings: `?` opens, k scrolls down, j up, G to bottom.
    handle_key(press('?'), &state, &tx).await;
    assert!(state.lock().unwrap().help_visible, "? opens the overlay");
    assert_eq!(state.lock().unwrap().help_scroll, 0, "opens at the top");

    handle_key(press('k'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().help_scroll, 1, "k scrolls down one");

    handle_key(press('K'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().help_scroll, 6, "K scrolls down five");

    handle_key(press('j'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().help_scroll, 5, "j scrolls back up");

    handle_key(press('g'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().help_scroll, 0, "g returns to top");

    // Must not underflow past the top.
    handle_key(press('j'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().help_scroll, 0, "no underflow at top");

    handle_key(press('?'), &state, &tx).await;
    assert!(!state.lock().unwrap().help_visible, "? closes it again");

    // Reopening starts at the top even after having scrolled.
    handle_key(press('?'), &state, &tx).await;
    handle_key(press('K'), &state, &tx).await;
    handle_key(press('?'), &state, &tx).await;
    handle_key(press('?'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().help_scroll, 0, "reopens at the top");
  }

  /// Keys that would otherwise act on the app must not leak through the
  /// overlay — pressing Space with help open should not toggle playback.
  #[tokio::test]
  async fn help_overlay_swallows_other_keys() {
    let state = test_state();
    let (tx, mut rx) = mpsc::channel::<IoEvent>(8);

    handle_key(press('?'), &state, &tx).await;
    handle_key(
      KeyEvent {
        code: KeyCode::Char(' '),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
      },
      &state,
      &tx,
    )
    .await;

    assert!(
      rx.try_recv().is_err(),
      "no IoEvent should be dispatched while the overlay is open"
    );
    assert!(state.lock().unwrap().help_visible, "overlay stays open");
  }
}
