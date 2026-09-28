use crate::app::{lock, ActiveBlock, AppState};
use crate::client::IoEvent;
use crate::config::keys::KeyBindings;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::Mutex;
use tokio::sync::mpsc;

pub(super) async fn handle(
  key: KeyEvent,
  state: &Mutex<AppState>,
  io_tx: &mpsc::Sender<IoEvent>,
  _keys: &KeyBindings,
) {
  match key.code {
    KeyCode::Esc => {
      let mut s = lock(state);
      s.pop_block();
    }
    KeyCode::Enter => {
      let query = lock(state).search_query.trim().to_string();
      if !query.is_empty() {
        // On a drop the input stays open with the query intact, so Enter
        // retries it.
        if super::send_io(state, io_tx, IoEvent::Search(query)) {
          let mut s = lock(state);
          s.active_block = ActiveBlock::SearchResults;
          s.block_history.clear();
        }
      }
    }
    KeyCode::Backspace => {
      lock(state).search_query.pop();
    }
    KeyCode::Char(c) => {
      lock(state).search_query.push(c);
    }
    _ => {}
  }
}
