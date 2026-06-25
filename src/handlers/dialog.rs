use crate::app::{AppState, DialogAction};
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::Mutex;
use tokio::sync::mpsc;

pub(super) async fn handle(
  key: KeyEvent,
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
  keys: &KeyBindings,
) {
  // Cancel: quit/back keys, 'n', or Esc.
  let cancel = keys.quit.matches(&key)
    || keys.back.matches(&key)
    || matches!(key.code, KeyCode::Char('n') | KeyCode::Char('N'));
  if cancel {
    let mut s = state.lock().unwrap();
    s.dialog = None;
    s.pop_block();
    return;
  }

  // Confirm: Enter, 'y'.
  let confirm = matches!(
    key.code,
    KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y')
  );
  if confirm {
    let action = {
      let mut s = state.lock().unwrap();
      let action = s.dialog.take().map(|d| d.action);
      s.pop_block();
      action
    };
    if let Some(DialogAction::UnfollowPlaylist { playlist_id }) = action {
      let _ = io_tx.send(IoEvent::UnfollowPlaylist(playlist_id)).await;
    }
  }
}
