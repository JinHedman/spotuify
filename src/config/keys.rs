use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer};

/// A single keystroke: a KeyCode plus modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
  pub code: KeyCode,
  pub modifiers: KeyModifiers,
}

impl KeyInput {
  pub fn matches(&self, event: &KeyEvent) -> bool {
    // Ignore the SHIFT bit on plain character keys — capitalization is
    // already reflected in KeyCode::Char. This keeps "Q" / "S" / etc.
    // matching the expected event.
    let mut mods = event.modifiers;
    if matches!(event.code, KeyCode::Char(_)) {
      mods.remove(KeyModifiers::SHIFT);
    }
    self.code == event.code && self.modifiers == mods
  }
}

/// Accepts either a single string or a list of strings.
#[derive(Debug, Clone)]
pub struct KeyList(pub Vec<KeyInput>);

impl KeyList {
  pub fn matches(&self, event: &KeyEvent) -> bool {
    self.0.iter().any(|k| k.matches(event))
  }

  pub fn single(s: &str) -> Self {
    Self(vec![parse(s).expect("invalid default key")])
  }

  pub fn many(items: &[&str]) -> Self {
    Self(
      items
        .iter()
        .map(|s| parse(s).expect("invalid default key"))
        .collect(),
    )
  }
}

impl<'de> Deserialize<'de> for KeyList {
  fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
    use serde::de::Error;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
      One(String),
      Many(Vec<String>),
    }
    let raw = Raw::deserialize(d)?;
    let strings: Vec<String> = match raw {
      Raw::One(s) => vec![s],
      Raw::Many(v) => v,
    };
    let parsed: Result<Vec<_>, _> = strings
      .iter()
      .map(|s| parse(s).map_err(D::Error::custom))
      .collect();
    Ok(KeyList(parsed?))
  }
}

fn parse(s: &str) -> Result<KeyInput, String> {
  let mut modifiers = KeyModifiers::NONE;
  let mut remaining = s.trim();

  loop {
    let lower = remaining.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("ctrl+") {
      modifiers |= KeyModifiers::CONTROL;
      remaining = &remaining[remaining.len() - rest.len()..];
    } else if let Some(rest) = lower.strip_prefix("alt+") {
      modifiers |= KeyModifiers::ALT;
      remaining = &remaining[remaining.len() - rest.len()..];
    } else if let Some(rest) = lower.strip_prefix("shift+") {
      modifiers |= KeyModifiers::SHIFT;
      remaining = &remaining[remaining.len() - rest.len()..];
    } else {
      break;
    }
  }

  let code = match remaining.to_ascii_lowercase().as_str() {
    "space" => KeyCode::Char(' '),
    "tab" => KeyCode::Tab,
    "backtab" | "shift+tab" => KeyCode::BackTab,
    "esc" | "escape" => KeyCode::Esc,
    "enter" | "return" => KeyCode::Enter,
    "backspace" => KeyCode::Backspace,
    "delete" | "del" => KeyCode::Delete,
    "home" => KeyCode::Home,
    "end" => KeyCode::End,
    "up" => KeyCode::Up,
    "down" => KeyCode::Down,
    "left" => KeyCode::Left,
    "right" => KeyCode::Right,
    "pageup" | "pgup" => KeyCode::PageUp,
    "pagedown" | "pgdn" => KeyCode::PageDown,
    _ => {
      // Single character (case-sensitive on the original input)
      let mut chars = remaining.chars();
      match (chars.next(), chars.next()) {
        (Some(c), None) => KeyCode::Char(c),
        _ => return Err(format!("unknown key name: {s:?}")),
      }
    }
  };

  Ok(KeyInput { code, modifiers })
}

/// All user-configurable keybindings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
  pub quit: KeyList,
  pub back: KeyList,
  pub help: KeyList,
  pub search: KeyList,
  pub device: KeyList,
  pub queue: KeyList,
  pub refresh: KeyList,
  pub play_pause: KeyList,
  pub next_track: KeyList,
  pub previous_track: KeyList,
  pub volume_up: KeyList,
  pub volume_down: KeyList,
  pub seek_forward: KeyList,
  pub seek_backward: KeyList,
  pub save_track: KeyList,
  pub save_album: KeyList,
  pub follow_artist: KeyList,
  pub delete_playlist: KeyList,
  pub theme_picker: KeyList,
  pub add_to_queue: KeyList,
  /// Directional navigation between the main panes (sidebar ↔ content, etc.).
  pub block_left: KeyList,
  pub block_right: KeyList,
  pub block_up: KeyList,
  pub block_down: KeyList,
  pub move_down: KeyList,
  pub move_up: KeyList,
  pub move_down_big: KeyList,
  pub move_up_big: KeyList,
  pub move_top: KeyList,
  pub move_bottom: KeyList,
  /// Activate / open the selected item. Enter plus vim-style `l`.
  pub activate: KeyList,
  /// Cycle tabs within a tabbed view (e.g. search results).
  pub search_tab_next: KeyList,
  pub search_tab_prev: KeyList,
}

impl Default for KeyBindings {
  fn default() -> Self {
    Self {
      // Esc is the only quit key — q / h go to `back` so they work for
      // popping out of a playlist/artist/album view.
      quit: KeyList::single("Esc"),
      back: KeyList::many(&["b", "Backspace", "q", "h"]),
      help: KeyList::single("?"),
      search: KeyList::single("/"),
      device: KeyList::single("d"),
      queue: KeyList::single("Q"),
      refresh: KeyList::single("r"),
      play_pause: KeyList::single("Space"),
      next_track: KeyList::single("n"),
      previous_track: KeyList::single("p"),
      volume_up: KeyList::many(&["+", "="]),
      volume_down: KeyList::many(&["-", "_"]),
      seek_forward: KeyList::single("]"),
      seek_backward: KeyList::single("["),
      save_track: KeyList::single("s"),
      save_album: KeyList::single("S"),
      follow_artist: KeyList::single("f"),
      delete_playlist: KeyList::single("D"),
      theme_picker: KeyList::single("t"),
      add_to_queue: KeyList::single("A"),
      // Directional pane navigation — mirrors vim/tmux. Uses the user's inverted
      // j/k (j=up, k=down) for vertical consistency with move_up/move_down.
      block_left: KeyList::single("ctrl+h"),
      block_right: KeyList::single("ctrl+l"),
      block_up: KeyList::single("ctrl+j"),
      block_down: KeyList::single("ctrl+k"),
      move_down: KeyList::many(&["k", "Down"]),
      move_up: KeyList::many(&["j", "Up"]),
      // Shift+J / Shift+K = jump by 5 (user has j/k inverted).
      move_down_big: KeyList::single("K"),
      move_up_big: KeyList::single("J"),
      move_top: KeyList::single("g"),
      move_bottom: KeyList::single("G"),
      // `l` doubles as "activate / enter" — vim-style go-into.
      activate: KeyList::many(&["Enter", "l"]),
      // Tab / BackTab cycle the tabs in the current view (e.g. search result
      // tabs). Arrow keys kept as alternates.
      search_tab_next: KeyList::many(&["Tab", "Right"]),
      search_tab_prev: KeyList::many(&["BackTab", "Left"]),
    }
  }
}
