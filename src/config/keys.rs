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

impl std::fmt::Display for KeyInput {
  /// Renders back into the syntax `parse` accepts, so what the UI shows is
  /// always something a user could type into `config.yml`.
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if self.modifiers.contains(KeyModifiers::CONTROL) {
      write!(f, "ctrl+")?;
    }
    if self.modifiers.contains(KeyModifiers::ALT) {
      write!(f, "alt+")?;
    }
    // Not SHIFT: `parse` folds it into the character for `KeyCode::Char`, and
    // `matches` strips it for the same reason. Emitting it would produce a
    // string that no longer round-trips.
    if self.modifiers.contains(KeyModifiers::SHIFT) && !matches!(self.code, KeyCode::Char(_)) {
      write!(f, "shift+")?;
    }
    match self.code {
      KeyCode::Char(' ') => write!(f, "Space"),
      KeyCode::Char(c) => write!(f, "{c}"),
      KeyCode::Tab => write!(f, "Tab"),
      KeyCode::BackTab => write!(f, "BackTab"),
      KeyCode::Esc => write!(f, "Esc"),
      KeyCode::Enter => write!(f, "Enter"),
      KeyCode::Backspace => write!(f, "Backspace"),
      KeyCode::Delete => write!(f, "Delete"),
      KeyCode::Home => write!(f, "Home"),
      KeyCode::End => write!(f, "End"),
      KeyCode::Up => write!(f, "Up"),
      KeyCode::Down => write!(f, "Down"),
      KeyCode::Left => write!(f, "Left"),
      KeyCode::Right => write!(f, "Right"),
      KeyCode::PageUp => write!(f, "PageUp"),
      KeyCode::PageDown => write!(f, "PageDown"),
      other => write!(f, "{other:?}"),
    }
  }
}

impl KeyList {
  /// The binding as the user would see it, e.g. `b / Backspace / q / h`.
  pub fn describe(&self) -> String {
    self
      .0
      .iter()
      .map(|k| k.to_string())
      .collect::<Vec<_>>()
      .join(" / ")
  }

  /// Just the first binding, for places with no room for the whole list.
  pub fn describe_first(&self) -> String {
    self.0.first().map(|k| k.to_string()).unwrap_or_default()
  }
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
  /// Shuffle on/off, and repeat cycling Off → Context → Track.
  pub shuffle: KeyList,
  pub repeat: KeyList,
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
      // `z` follows the mpd/ncmpcpp convention for random. `R` because `r` is
      // already refresh, and shift-r keeps the mnemonic.
      shuffle: KeyList::single("z"),
      repeat: KeyList::single("R"),
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

impl KeyBindings {
  /// Every action with its section and a human label, in display order.
  ///
  /// The single source for both the help overlay and the status-line legend.
  /// Both used to hold their own hardcoded key strings, which meant they
  /// stated the defaults rather than the user's actual bindings — the help
  /// still said `z` after you rebound shuffle.
  pub fn all(&self) -> Vec<(&'static str, &'static str, &KeyList)> {
    vec![
      ("Navigation", "focus pane left", &self.block_left),
      ("Navigation", "focus pane right", &self.block_right),
      ("Navigation", "focus pane up", &self.block_up),
      ("Navigation", "focus pane down", &self.block_down),
      ("Navigation", "move selection down", &self.move_down),
      ("Navigation", "move selection up", &self.move_up),
      ("Navigation", "move down by 5", &self.move_down_big),
      ("Navigation", "move up by 5", &self.move_up_big),
      ("Navigation", "top of list", &self.move_top),
      ("Navigation", "bottom of list", &self.move_bottom),
      ("Navigation", "activate / open", &self.activate),
      ("Navigation", "back", &self.back),
      ("Navigation", "quit", &self.quit),
      ("Search", "open search", &self.search),
      ("Search", "next result tab", &self.search_tab_next),
      ("Search", "previous result tab", &self.search_tab_prev),
      ("Playback", "play / pause", &self.play_pause),
      ("Playback", "next track", &self.next_track),
      ("Playback", "previous track", &self.previous_track),
      ("Playback", "volume up", &self.volume_up),
      ("Playback", "volume down", &self.volume_down),
      ("Playback", "seek forward", &self.seek_forward),
      ("Playback", "seek backward", &self.seek_backward),
      ("Playback", "toggle shuffle", &self.shuffle),
      ("Playback", "cycle repeat", &self.repeat),
      ("Playback", "refresh playback", &self.refresh),
      ("Playback", "select device", &self.device),
      ("Playback", "show queue", &self.queue),
      ("Playback", "add to queue", &self.add_to_queue),
      ("Library", "save / unsave track", &self.save_track),
      ("Library", "save / unsave album", &self.save_album),
      ("Library", "follow / unfollow artist", &self.follow_artist),
      ("Library", "remove playlist", &self.delete_playlist),
      ("Appearance", "change theme", &self.theme_picker),
      ("Help", "toggle help", &self.help),
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The property that makes displaying bindings safe: anything shown in the
  /// UI must parse back to the same key, or the help would be telling users
  /// to type something the parser rejects.
  #[test]
  fn every_default_binding_round_trips() {
    let keys = KeyBindings::default();
    for (_, name, list) in keys.all() {
      for input in &list.0 {
        let shown = input.to_string();
        let reparsed =
          parse(&shown).unwrap_or_else(|e| panic!("{name}: {shown:?} does not parse back: {e}"));
        assert_eq!(
          &reparsed, input,
          "{name}: {shown:?} parsed back to a different key"
        );
      }
    }
  }

  #[test]
  fn modifiers_render_in_the_parsed_form() {
    assert_eq!(parse("ctrl+h").unwrap().to_string(), "ctrl+h");
    assert_eq!(parse("Space").unwrap().to_string(), "Space");
    assert_eq!(parse("Esc").unwrap().to_string(), "Esc");
    assert_eq!(parse("Backspace").unwrap().to_string(), "Backspace");
    assert_eq!(parse("?").unwrap().to_string(), "?");
    // Capitals keep their case rather than becoming shift+.
    assert_eq!(parse("Q").unwrap().to_string(), "Q");
  }

  #[test]
  fn describe_joins_alternatives() {
    let list = KeyList::many(&["b", "Backspace", "q"]);
    assert_eq!(list.describe(), "b / Backspace / q");
    assert_eq!(list.describe_first(), "b");
  }

  /// Every action must be reachable through `all()`, or the help overlay
  /// would quietly omit whichever one was forgotten.
  #[test]
  fn all_covers_every_action() {
    let keys = KeyBindings::default();
    let listed = keys.all().len();
    // One entry per public KeyList field.
    let fields = 35;
    assert_eq!(
      listed, fields,
      "all() lists {listed} actions; update it when adding a binding"
    );
    for (_, name, list) in keys.all() {
      assert!(!list.0.is_empty(), "{name} has no default binding");
    }
  }
}
