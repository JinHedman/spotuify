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

use crate::app::{lock, ActiveBlock, AppState};
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crate::config::user::UserConfig;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self, error::TrySendError};
use tracing::warn;

/// Queue `event` for the network task from the UI path. Never waits.
/// Returns whether it was queued.
///
/// Every handler runs on the UI loop, and Repeat events reach handlers, so an
/// awaiting send would freeze drawing and key handling as soon as a stalled
/// network task let the channel fill. A full channel drops the event and
/// says so instead. A closed channel is not reported: the main loop notices
/// the network task has ended and exits with its error.
///
/// Callers that change the view in anticipation of the response (open a
/// pane, switch to results) must only do so when this returns true, or the
/// new pane shows the previous list as if it were the answer.
///
/// Must not be called while holding the state lock — the full path takes it.
pub(crate) fn send_io(
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
  event: IoEvent,
) -> bool {
  match io_tx.try_send(event) {
    Ok(()) => true,
    Err(TrySendError::Closed(_)) => false,
    Err(TrySendError::Full(event)) => {
      warn!(name = event.name(), "network channel full, dropping action");
      lock(state).note_dropped_action();
      false
    }
  }
}

pub enum KeyOutcome {
  Continue,
  Quit,
}

pub async fn handle_key(
  key: KeyEvent,
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
) -> KeyOutcome {
  let config: Arc<UserConfig> = lock(state).config.clone();
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
  if lock(state).help_visible {
    let mut s = lock(state);
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

  let active = lock(state).active_block;

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
    let mut s = lock(state);
    s.help_visible = true;
    // Always open at the top rather than wherever it was last left.
    s.help_scroll = 0;
    return KeyOutcome::Continue;
  }
  if keys.search.matches(&key) {
    lock(state).push_block(ActiveBlock::SearchInput);
    return KeyOutcome::Continue;
  }
  if keys.device.matches(&key) {
    if send_io(state, io_tx, IoEvent::GetDevices) {
      lock(state).push_block(ActiveBlock::SelectDevice);
    }
    return KeyOutcome::Continue;
  }
  if keys.queue.matches(&key) {
    if send_io(state, io_tx, IoEvent::GetQueue) {
      lock(state).push_block(ActiveBlock::Queue);
    }
    return KeyOutcome::Continue;
  }
  if keys.play_pause.matches(&key) {
    let is_playing = lock(state).is_playing();
    let ev = if is_playing {
      IoEvent::PausePlayback
    } else {
      IoEvent::ResumePlayback
    };
    send_io(state, io_tx, ev);
    return KeyOutcome::Continue;
  }
  if keys.next_track.matches(&key) {
    send_io(state, io_tx, IoEvent::NextTrack);
    return KeyOutcome::Continue;
  }
  if keys.previous_track.matches(&key) {
    send_io(state, io_tx, IoEvent::PreviousTrack);
    return KeyOutcome::Continue;
  }
  if keys.volume_up.matches(&key) {
    let v = lock(state).current_volume();
    send_io(
      state,
      io_tx,
      IoEvent::ChangeVolume(v.saturating_add(behavior.volume_step).min(100)),
    );
    return KeyOutcome::Continue;
  }
  if keys.volume_down.matches(&key) {
    let v = lock(state).current_volume();
    send_io(
      state,
      io_tx,
      IoEvent::ChangeVolume(v.saturating_sub(behavior.volume_step)),
    );
    return KeyOutcome::Continue;
  }
  if keys.seek_backward.matches(&key) {
    let progress = lock(state).current_progress_ms();
    if let Some(p) = progress {
      send_io(
        state,
        io_tx,
        IoEvent::Seek((p - behavior.seek_step_ms).max(0)),
      );
    }
    return KeyOutcome::Continue;
  }
  if keys.seek_forward.matches(&key) {
    let progress = lock(state).current_progress_ms();
    if let Some(p) = progress {
      send_io(state, io_tx, IoEvent::Seek(p + behavior.seek_step_ms));
    }
    return KeyOutcome::Continue;
  }
  if keys.shuffle.matches(&key) {
    send_io(state, io_tx, IoEvent::ToggleShuffle);
    return KeyOutcome::Continue;
  }
  if keys.repeat.matches(&key) {
    send_io(state, io_tx, IoEvent::CycleRepeat);
    return KeyOutcome::Continue;
  }
  if keys.refresh.matches(&key) {
    let pending = lock(state).pending_io.clone();
    pending.send(io_tx, IoEvent::GetCurrentPlayback);
    return KeyOutcome::Continue;
  }
  if keys.save_track.matches(&key) {
    let track_id = lock(state).current_track_id();
    if let Some(id) = track_id {
      send_io(state, io_tx, IoEvent::ToggleSaveTrack(id));
    }
    return KeyOutcome::Continue;
  }
  if keys.save_album.matches(&key) {
    let album_id = lock(state).current_album_id();
    if let Some(id) = album_id {
      send_io(state, io_tx, IoEvent::ToggleSaveAlbum(id));
    }
    return KeyOutcome::Continue;
  }
  if keys.follow_artist.matches(&key) {
    let artist_id = lock(state).current_artist_id();
    if let Some(id) = artist_id {
      send_io(state, io_tx, IoEvent::ToggleFollowArtist(id));
    }
    return KeyOutcome::Continue;
  }
  if keys.theme_picker.matches(&key) {
    let mut s = lock(state);
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
    let mut s = lock(state);
    s.active_block = s.active_block.go_left();
    return KeyOutcome::Continue;
  }
  if keys.block_right.matches(&key) {
    let mut s = lock(state);
    s.active_block = s.active_block.go_right();
    return KeyOutcome::Continue;
  }
  if keys.block_up.matches(&key) {
    let mut s = lock(state);
    s.active_block = s.active_block.go_up();
    return KeyOutcome::Continue;
  }
  if keys.block_down.matches(&key) {
    let mut s = lock(state);
    s.active_block = s.active_block.go_down();
    return KeyOutcome::Continue;
  }
  if keys.back.matches(&key) {
    let mut s = lock(state);
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

  /// A stalled network task fills the channel. Handlers must drop the event
  /// and return rather than wait for capacity — an awaiting send here would
  /// never complete and this test would hang.
  #[tokio::test]
  async fn full_channel_drops_the_action_without_blocking() {
    let state = test_state();
    let (tx, mut rx) = mpsc::channel::<IoEvent>(1);
    tx.try_send(IoEvent::GetQueue).unwrap();

    for _ in 0..3 {
      handle_key(press('n'), &state, &tx).await;
    }

    assert_eq!(
      state.lock().unwrap().notice(),
      Some(AppState::DROPPED_NOTICE)
    );
    assert!(matches!(rx.try_recv(), Ok(IoEvent::GetQueue)));
    assert!(
      rx.try_recv().is_err(),
      "dropped events are not queued later"
    );
  }

  /// A dropped fetch must not open its pane: the new view would show the
  /// previous list as if it were the answer.
  #[tokio::test]
  async fn dropped_fetch_does_not_navigate() {
    let state = test_state();
    state.lock().unwrap().active_block = ActiveBlock::Library;
    let (tx, mut rx) = mpsc::channel::<IoEvent>(1);
    tx.try_send(IoEvent::GetQueue).unwrap();

    handle_key(press('l'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().active_block, ActiveBlock::Library);

    // With room in the channel the same key opens the pane.
    rx.try_recv().unwrap();
    handle_key(press('l'), &state, &tx).await;
    assert_eq!(state.lock().unwrap().active_block, ActiveBlock::TrackTable);
    assert!(matches!(rx.try_recv(), Ok(IoEvent::GetSavedTracks)));
  }

  /// A dropped search leaves the input open with the query, so Enter retries.
  #[tokio::test]
  async fn dropped_search_keeps_the_input_open() {
    let state = test_state();
    {
      let mut s = state.lock().unwrap();
      s.push_block(ActiveBlock::SearchInput);
      s.search_query = "abba".to_string();
    }
    let (tx, _rx) = mpsc::channel::<IoEvent>(1);
    tx.try_send(IoEvent::GetQueue).unwrap();
    let enter = KeyEvent {
      code: KeyCode::Enter,
      ..press('x')
    };

    handle_key(enter, &state, &tx).await;
    let s = state.lock().unwrap();
    assert_eq!(s.active_block, ActiveBlock::SearchInput);
    assert_eq!(s.search_query, "abba");
  }

  /// Holding a key during a stall repeats the drop many times a second; the
  /// notice is posted once per NOTICE_TTL rather than on every repeat.
  #[test]
  fn dropped_notice_is_rate_limited() {
    let state = test_state();
    let (tx, _rx) = mpsc::channel::<IoEvent>(1);
    tx.try_send(IoEvent::GetQueue).unwrap();

    send_io(&state, &tx, IoEvent::NextTrack);
    let first = state.lock().unwrap().dropped_notice_at;
    assert!(first.is_some());
    state.lock().unwrap().set_notice("something else");
    send_io(&state, &tx, IoEvent::NextTrack);
    assert_eq!(state.lock().unwrap().dropped_notice_at, first);
    assert_eq!(state.lock().unwrap().notice(), Some("something else"));

    // Once the window has passed it is posted again.
    state.lock().unwrap().dropped_notice_at = first.map(|t| t - AppState::NOTICE_TTL);
    send_io(&state, &tx, IoEvent::NextTrack);
    assert_eq!(
      state.lock().unwrap().notice(),
      Some(AppState::DROPPED_NOTICE)
    );
  }

  /// The network task can panic while holding the lock after the main loop's
  /// poison check has passed. A handler must not panic a second time on the
  /// poisoned mutex, or the UI's panic would bury the network task's error.
  #[tokio::test]
  async fn poisoned_state_does_not_panic_handlers() {
    let state = Arc::new(test_state());
    let poisoner = Arc::clone(&state);
    let _ = std::thread::spawn(move || {
      let _guard = poisoner.lock().unwrap();
      panic!("network task panicked while holding the lock");
    })
    .join();
    assert!(state.is_poisoned());

    let (tx, _rx) = mpsc::channel::<IoEvent>(8);
    for c in ['?', 'k', '?', 'n', '/'] {
      handle_key(press(c), &state, &tx).await;
    }
    assert!(!lock(&state).help_visible);
  }
}
