mod app;
mod auth;
mod client;
mod config;
mod handlers;
mod ui;

use anyhow::Result;
use app::{lock, AppState};
use clap::Parser;
use client::{IoEvent, Network};
use config::client::ClientConfig;
use config::user::UserConfig;
use config::{
  client_config_path, presets, selected_theme_path, time_of_day_path, token_cache_path,
  user_config_path,
};
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use handlers::KeyOutcome;
use ratatui::DefaultTerminal;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinHandle};
use tokio::time;
use tracing::warn;

const BANNER: &str = r"
   ____             _
  / ___| _ __   ___ | |_
  \___ \| '_ \ / _ \| __|
   ___) | |_) | (_) | |_
  |____/| .__/ \___/ \__|
        |_|

  Terminal client for Spotify
";

/// Redraw interval while a theme fade is in flight — roughly 30fps, enough
/// for a blend to read as continuous motion.
const TRANSITION_FRAME_MS: u64 = 33;

/// How long to wait for the network task to drain after the terminal has been
/// restored, before giving up and exiting anyway.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(1500);

#[derive(Parser, Debug)]
#[command(
  name = "spot",
  version,
  about = "A terminal client for Spotify",
  before_help = BANNER
)]
struct Cli {}

#[tokio::main]
async fn main() -> Result<()> {
  let _cli = Cli::parse();

  install_panic_hook();

  let client_path = client_config_path()?;
  let cache_path = token_cache_path()?;
  let user_cfg_path = user_config_path()?;
  let client_cfg = ClientConfig::load_or_bootstrap(&client_path)?;
  // A malformed config.yml no longer stops startup: defaults are used and the
  // problem is shown in the UI. Refusing to launch over one bad line in a file
  // where every field is optional is the wrong trade, and the app is the only
  // place the result is visible.
  let loaded = UserConfig::load(&user_cfg_path);
  let config_problem = loaded.problem;
  let user_cfg = Arc::new(loaded.config);

  let spotify = auth::build_client(&client_cfg, cache_path);
  auth::authenticate(&spotify).await?;

  let state = Arc::new(Mutex::new(AppState::new(Arc::clone(&user_cfg))));

  if let Some(problem) = config_problem {
    warn!(%problem, "falling back to default configuration");
    state.lock().unwrap().set_notice(problem);
  }

  // If the user picked a preset in a previous session, apply it now — overrides
  // the theme that came in from config.yml. A malformed or stale file is
  // silently ignored; the built-in default stays in place.
  if let Ok(path) = selected_theme_path() {
    if let Ok(raw) = std::fs::read_to_string(&path) {
      // By index, so a persisted "Decade" choice restores the mode and not
      // just a palette. Zero duration: fading in from the built-in default at
      // startup would look like a glitch.
      if let Some(index) = presets::index_by_name(raw.trim()) {
        state.lock().unwrap().select_preset(index, Duration::ZERO);
      }
    }
  }

  // Restore the after-dark modifier, which the picker persists separately —
  // it layers on top of the theme rather than being one, so it cannot live in
  // .selected_theme. Malformed content is ignored, same as the theme file.
  if let Ok(path) = time_of_day_path() {
    if let Ok(raw) = std::fs::read_to_string(&path) {
      if let Ok(value) = raw.trim().parse::<f32>() {
        state.lock().unwrap().time_of_day_shift = value.clamp(0.0, 1.0);
      }
    }
  }

  let (io_tx, io_rx) = mpsc::channel::<IoEvent>(64);

  let network = Network::new(spotify, Arc::clone(&state));
  let mut network_handle = tokio::spawn(network.run(io_rx));

  let terminal = ratatui::init();
  let result = run(
    terminal,
    Arc::clone(&state),
    io_tx.clone(),
    Arc::clone(&user_cfg),
    &mut network_handle,
  )
  .await;
  // Restored before `result` is returned, so an error (including the network
  // task dying) prints on a normal terminal rather than inside the alt screen.
  ratatui::restore();

  // `try_send`: an awaiting send on a full channel would hang quit before the
  // grace timeout below ever started. A missed sentinel is covered by closing
  // the channel and then by the timeout.
  let _ = io_tx.try_send(IoEvent::Shutdown);
  // Drop our sender so the channel closes even if the sentinel was missed
  // (a full channel, or a task that already exited). Without this, `recv()`
  // in the network task blocks forever and the await below never returns.
  drop(io_tx);
  // Bounded: a cover render can be mid-ffmpeg when we quit, and waiting on it
  // would stall the exit by up to COVER_TIMEOUT. Nothing in the network task
  // holds unsaved state — cache writes are atomic via rename — so abandoning
  // it is safe, and a prompt exit matters more than a tidy join.
  //
  // Skipped when the task has already ended: `run` may have consumed its
  // output, and there is nothing left to wait for.
  if !network_handle.is_finished()
    && tokio::time::timeout(SHUTDOWN_GRACE, network_handle)
      .await
      .is_err()
  {
    tracing::warn!("network task did not stop within {SHUTDOWN_GRACE:?}");
  }

  result
}

async fn run(
  mut terminal: DefaultTerminal,
  state: Arc<Mutex<AppState>>,
  io_tx: mpsc::Sender<IoEvent>,
  user_cfg: Arc<UserConfig>,
  network: &mut JoinHandle<()>,
) -> Result<()> {
  let pending = lock(&state).pending_io.clone();
  pending.send(&io_tx, IoEvent::GetCurrentPlayback);
  handlers::send_io(&state, &io_tx, IoEvent::GetPlaylists);

  let mut events = EventStream::new();
  let mut poll = time::interval(Duration::from_millis(user_cfg.behavior.poll_interval_ms));
  poll.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
  loop {
    // A panic in the network task while it held the lock poisons it. Stop
    // here and report the task's exit, which explains why. A panic that lands
    // mid-iteration is survived by `app::lock`, which recovers the guard, so
    // the handler finishes and this check catches it on the next pass.
    if state.is_poisoned() {
      let exit = time::timeout(SHUTDOWN_GRACE, &mut *network).await;
      return Err(match exit {
        Ok(res) => network_exit_error(res),
        Err(_) => anyhow::anyhow!("network task failed and left shared state unusable"),
      });
    }

    terminal.draw(|f| ui::draw(f, &state))?;

    // Redraw faster while anything is mid-animation. At the default 200ms
    // tick a 350ms fade would paint two intermediate frames, which reads as a
    // stutter rather than a transition. Reverts to the configured tick as
    // soon as nothing is animating, so the idle cost is unchanged.
    let fading = lock(&state).needs_fast_redraw();
    let redraw_in = if fading {
      Duration::from_millis(TRANSITION_FRAME_MS)
    } else {
      Duration::from_millis(user_cfg.behavior.tick_rate_ms)
    };

    tokio::select! {
      // The network task only ends on Shutdown, which is sent after this
      // loop returns. Ending earlier means it panicked; without this branch
      // the UI would keep drawing stale state with nothing serving requests.
      res = &mut *network => return Err(network_exit_error(res)),
      _ = time::sleep(redraw_in) => {}
      _ = poll.tick() => {
        // Never awaits: if the network task is stalled the channel fills, and
        // an awaiting send here would stop redraws and key handling with it.
        // At most one poll is queued; later ticks are dropped until it is
        // dequeued.
        pending.send(&io_tx, IoEvent::GetCurrentPlayback);
      }
      maybe_evt = events.next() => {
        let Some(evt) = maybe_evt else { return Ok(()) };
        if let Event::Key(key) = evt? {
          if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
          }
          if matches!(handlers::handle_key(key, &state, &io_tx).await, KeyOutcome::Quit) {
            return Ok(());
          }
        }
      }
    }
  }
}

/// Describe how the network task ended, for the message printed after the
/// terminal is restored.
fn network_exit_error(res: Result<(), JoinError>) -> anyhow::Error {
  match res {
    Ok(()) => anyhow::anyhow!("network task stopped unexpectedly"),
    Err(err) if err.is_panic() => {
      let payload = err.into_panic();
      let msg = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string());
      anyhow::anyhow!("network task panicked: {msg}")
    }
    Err(err) => anyhow::anyhow!("network task ended: {err}"),
  }
}

fn install_panic_hook() {
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    ratatui::restore();
    default_hook(info);
  }));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn network_panic_is_reported_with_its_message() {
    let res = tokio::spawn(async { panic!("boom") }).await;
    let msg = network_exit_error(res).to_string();
    assert_eq!(msg, "network task panicked: boom");
  }

  #[tokio::test]
  async fn early_clean_exit_is_still_an_error() {
    let res = tokio::spawn(async {}).await;
    assert_eq!(
      network_exit_error(res).to_string(),
      "network task stopped unexpectedly"
    );
  }
}
