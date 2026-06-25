use crate::app::{ActiveBlock, AppState};
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
      let mut s = state.lock().unwrap();
      s.pop_block();
    }
    KeyCode::Enter => {
      let query = state.lock().unwrap().search_query.trim().to_string();
      if !query.is_empty() {
        let _ = io_tx.send(IoEvent::Search(query)).await;
        let mut s = state.lock().unwrap();
        s.active_block = ActiveBlock::SearchResults;
        s.block_history.clear();
      }
    }
    KeyCode::Backspace => {
      state.lock().unwrap().search_query.pop();
    }
    KeyCode::Char(c) => {
      state.lock().unwrap().search_query.push(c);
    }
    _ => {}
  }
}
