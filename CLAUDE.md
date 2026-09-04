# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Rust TUI Spotify remote. Binary name is `spot`; crate and config directory are `spotuify`. Only controls playback via the Spotify Web API — actual audio comes from any Spotify Connect device on the account.

- Design rationale and intentional deletions: `PLAN.md`
- User-facing docs, keybindings, config schema: `README.md`

## Commands

```bash
cargo build --release            # release binary at target/release/spot
cargo run                        # debug run (launches TUI)
cargo fmt --all                  # 2-space indents, edition 2021 (see rustfmt.toml)
cargo clippy --all-targets -- -D warnings   # CI treats warnings as errors
cargo check --all-targets
cargo test --all-targets
cargo test <name>                # run a single test by substring
```

Linux system deps for the `arboard` clipboard crate and OpenSSL: `libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev pkg-config`. CI (`.github/workflows/ci.yml`) runs fmt + clippy + check + test with `RUSTFLAGS=-D warnings`.

### Installing the `spot` binary on this machine

The user runs `spot` from `~/.local/bin/spot` (on PATH). After changes that the user wants to use, rebuild and copy:

```bash
cargo build --release && cp target/release/spot ~/.local/bin/spot
```

`cargo install --path .` would put it at `~/.cargo/bin/spot` instead — don't switch locations without asking.

## Architecture

Single `#[tokio::main]` runtime with two actors and an event stream wired together by `tokio::select!` in `src/main.rs`:

1. **UI task** (main loop in `main.rs::run`) — owns the terminal, draws via `ui::draw`, consumes `crossterm::event::EventStream`, and dispatches keys through `handlers::handle_key`. Only `KeyEventKind::Press | Repeat` reaches handlers — release events are filtered at the `select!` branch.
2. **Network task** (`src/client/mod.rs` — `Network::run`) — `tokio::spawn`'d consumer of `mpsc::Receiver<IoEvent>`. Calls `rspotify` (`AuthCodeSpotify`) and writes results back into shared state.

Shared state is `Arc<Mutex<AppState>>` (note: the plan mentions `RwLock`, but the implementation uses `Mutex` — don't change this without a reason). All state lives in `src/app/mod.rs::AppState`.

Transient messages go through `AppState::set_notice` and render on the bottom status row (`ui::legend`), which yields its key legend for `NOTICE_TTL`. They expire on read rather than being cleared by a later success — a playback poll says nothing about, say, a config parse error. Never render a message by replacing a pane.

`main.rs::install_panic_hook` wraps the default panic hook with `ratatui::restore()` so a panic doesn't leave the terminal in raw/alt-screen mode. Preserve this if you touch startup.

Auth lives in `src/auth/mod.rs` — `build_client` constructs the `AuthCodeSpotify` with the token cache path, `authenticate` runs the OAuth dance (opens the browser, reads the pasted redirect URL). Runs once at startup before the tokio actors spin up.

### The `IoEvent` contract

`src/client/mod.rs::IoEvent` is the full contract between UI and network. **One variant per Spotify call.** To add a new Spotify interaction:

1. Add an `IoEvent` variant.
2. Match it in `Network::dispatch` and implement the handler method.
3. Dispatch it from a handler via `io_tx.send(IoEvent::…).await`.

`IoEvent::Shutdown` is sent from `main.rs` after terminal teardown so the network task drains cleanly.

### Block / route model

Navigation is a stack of `ActiveBlock`s (`src/app/route.rs`) kept in `AppState.block_history`. `push_block` / `pop_block` on `AppState` are the only legitimate way to change focus. Handlers are one file per block under `src/handlers/`; the matching renderer lives under `src/ui/`. Overlays (`help_visible`) and modal blocks (`SearchInput`, `SelectDevice`, `Queue`) are handled before the global key map in `handlers/mod.rs::handle_key`.

### Keybindings

Every handler compares against `app.config.keys.*` (`src/config/keys.rs`) — **never hard-code keys** except `Ctrl+C` (hard-wired quit). Each action accepts a single key or a list; users override via `$CONFIG_DIR/spotuify/config.yml`.

`KeyBindings::all()` returns `(section, label, keys)` for every action and is the single source for both the help overlay (`ui::help`) and the status-line legend (`ui::legend`) — **never hard-code key strings in either**. Adding a binding means adding a field *and* an `all()` row; a test asserts the counts match. `KeyInput` implements `Display` in the syntax `parse` accepts, and a test round-trips every default so the UI can only show something a user could actually type. The parser has no function-key names (`F1`…), so bindings are limited to named keys and single characters.

### Config & auth

- `src/config/mod.rs` resolves paths via the `directories` crate under `ProjectDirs::from("io", "", "spotuify")`.
- `client.yml` (credentials) — `ClientConfig::load_or_bootstrap` runs the interactive first-run wizard if missing.
- `config.yml` (theme/behavior/keys) — `UserConfig::load` returns a `LoadedConfig { config, problem }` and never fails; a malformed file falls back to defaults and reports the parse error through `problem`, which `main.rs` puts into `last_error`. Missing file = built-in defaults, missing fields = per-field defaults. `load_or_default` is `#[cfg(test)]` only, so production cannot bypass the reporting.
- Token cache at `.token_cache.json` in the same directory. Auth is OAuth auth-code via `rspotify` 0.16 with automatic refresh.
- Theme presets live in `src/config/presets.rs`; the theme picker (`t`) writes the chosen preset name to `.selected_theme` in the same config dir. `main.rs` reads that file at startup *after* applying `config.yml`, so `.selected_theme` overrides `config.yml`'s theme. Malformed/stale content is silently ignored.

### Playbar smoothness

`AppState::extrapolated_progress_ms` adds `Instant::elapsed` since the last poll to the cached `progress` so the progress bar moves between polls. The `tick_rate_ms` (redraw) and `poll_interval_ms` (network) intervals are separate and both come from `UserConfig.behavior`.

## Spotify API deprecations (hard constraints)

Spotify deprecated these endpoints on **2024-11-27** for apps without legacy extended-mode access. Do **not** add features that depend on them:

- Audio analysis / audio features
- Recommendations (`GET /recommendations`)
- Related artists
- Featured playlists
- Algorithmic playlists (Discover Weekly, Release Radar, Made For You)
- `GET /artists/{id}/top-tracks` is flagged deprecated in rspotify 0.16 — current code keeps it under `#[allow(deprecated)]` and it only works on legacy-access accounts.

See `PLAN.md` §3 and README "Known limitations" for the full list.

## Conventions

- Rust edition 2021, 2-space indents (`rustfmt.toml`).
- `anyhow::Result` at boundaries, `thiserror` for typed errors where needed.
- Logging via `tracing` — the TUI owns stdout, so never `println!` from runtime code paths.
- Keep handlers thin: parse key → mutate state or send `IoEvent`. Business logic that hits Spotify belongs in `Network`.
