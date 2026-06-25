# Spotuify — rewrite plan

A modern Rust TUI Spotify remote, ported from `spotify-tui` conventions but built on current crates. Drops the audio-analysis ("pitches") view. Binary name: `spotuify`.

## 1. Stack

| Concern | Old (spotify-tui) | New (spotuify) |
|---|---|---|
| TUI renderer | `tui` 0.16 (unmaintained) | **`ratatui` 0.30** |
| Terminal backend | `crossterm` 0.20 | **`crossterm` 0.29** (via `ratatui-crossterm`) |
| Spotify client | `rspotify` 0.10 | **`rspotify` 0.16** |
| Async runtime | `tokio` 0.2 (two `#[tokio::main]` on two OS threads) | **`tokio` 1.52** (single runtime, tasks) |
| CLI parser | `clap` 2.33 | **`clap` 4.6** (derive macros) |
| Rust edition | 2018 | **2021** |
| Error handling | `anyhow` + `failure` mix | **`anyhow` + `thiserror`** |
| Logging | none | **`tracing` + `tracing-subscriber`** (file log, since TUI owns stdout) |
| Config | `serde_yaml` 0.8 | **`serde` + `serde_yaml` 0.9** (or `toml` — decide early, see §7) |
| Clipboard | `arboard` | **`arboard`** (latest) |

## 2. Architectural model

Keep the mental model from spotify-tui, modernize the plumbing.

**Old**: UI thread + separate OS thread for network, each with its own `#[tokio::main]` runtime, wired by `std::sync::mpsc`. Shared state behind `Arc<Mutex<App>>`. Needed because `rspotify` 0.10 was async but the UI loop was blocking.

**New**: Single `#[tokio::main]`. One runtime. Three concurrent tasks:

1. **UI task** — owns the terminal, runs the draw/event loop. Sends `IoEvent`s.
2. **Network task** — `while let Some(ev) = io_rx.recv().await` loop, calls `rspotify`. Writes results back into shared `AppState`.
3. **Event task** — reads `crossterm::event::EventStream` (now `futures::Stream` friendly), forwards `KeyEvent`/`Resize`/tick into the UI task.

Channels: `tokio::sync::mpsc` for both directions. Shared state: `Arc<RwLock<AppState>>` (write-rare from network, read-often from UI). Navigation stack, current route, block focus unchanged.

Why single runtime: removes the two-runtime hack, makes cancellation clean, lets us use `tokio::select!` for tick + event + shutdown.

## 3. Project layout

```
spotuify/
├── Cargo.toml
├── README.md
├── rustfmt.toml           # tab_spaces = 2, edition = "2021"
├── rust-toolchain.toml    # pin stable
├── .github/workflows/
│   ├── ci.yml             # fmt + clippy + check + test
│   └── release.yml        # tag-driven binary + crates.io
├── src/
│   ├── main.rs            # entry, clap dispatch, runtime bootstrap
│   ├── lib.rs             # re-exports for integration tests
│   ├── app/
│   │   ├── mod.rs         # AppState, Route, RouteId, ActiveBlock
│   │   ├── pagination.rs  # ScrollableResultPages<T>
│   │   └── library.rs     # static library sidebar entries
│   ├── auth/
│   │   ├── mod.rs         # OAuth flow (PKCE), token cache, refresh
│   │   └── redirect.rs    # loopback HTTP server for callback
│   ├── client/
│   │   ├── mod.rs         # Network task, IoEvent enum, dispatcher
│   │   └── events.rs      # IoEvent variants (one per Spotify call)
│   ├── config/
│   │   ├── mod.rs         # ClientConfig + UserConfig loader
│   │   ├── client.rs      # client.yml (id/secret/port)
│   │   ├── user.rs        # config.yml (theme, behavior, keys)
│   │   └── theme.rs       # Theme struct, defaults
│   ├── keys/
│   │   └── mod.rs         # KeyBindings struct, default map
│   ├── handlers/
│   │   ├── mod.rs         # handle_app — global keys + dispatch to block
│   │   ├── common.rs      # shared movement helpers (j/k/g/G/pg)
│   │   ├── input.rs       # search input box (eats most keys)
│   │   ├── home.rs
│   │   ├── library.rs
│   │   ├── track_table.rs
│   │   ├── album_list.rs
│   │   ├── album_tracks.rs
│   │   ├── artist.rs
│   │   ├── artists.rs
│   │   ├── artist_albums.rs
│   │   ├── playlist.rs
│   │   ├── search_results.rs
│   │   ├── podcasts.rs
│   │   ├── episode_table.rs
│   │   ├── recently_played.rs
│   │   ├── select_device.rs
│   │   ├── help_menu.rs
│   │   ├── playbar.rs
│   │   ├── dialog.rs      # confirm prompts (delete playlist, etc.)
│   │   ├── error_screen.rs
│   │   ├── basic_view.rs  # fallback for tiny terminals
│   │   └── empty.rs
│   ├── ui/
│   │   ├── mod.rs         # draw() entrypoint, branches on active_block
│   │   ├── layout.rs      # main three-pane layout + responsive breakpoints
│   │   ├── widgets/       # reusable widgets (table, list, gauge, etc.)
│   │   │   ├── mod.rs
│   │   │   ├── track_table.rs
│   │   │   ├── playbar.rs
│   │   │   ├── library.rs
│   │   │   ├── playlists.rs
│   │   │   ├── search.rs
│   │   │   └── help.rs
│   │   ├── basic_view.rs
│   │   ├── help.rs        # get_help_docs()
│   │   ├── dialog.rs
│   │   └── util.rs        # SMALL_TERMINAL_* thresholds, color helpers
│   ├── cli/
│   │   ├── mod.rs         # entry: handle_matches()
│   │   ├── command.rs     # clap 4 derive structs
│   │   ├── playback.rs    # spt playback
│   │   ├── play.rs        # spt play
│   │   ├── list.rs        # spt list
│   │   └── search.rs      # spt search
│   ├── banner.rs          # ASCII art startup banner
│   └── util.rs            # misc helpers (ms→mm:ss, truncate, etc.)
└── tests/
    ├── config_parse.rs
    └── key_dispatch.rs
```

Intentional deletions vs spotify-tui (all driven by Spotify's 2024-11-27 Web API deprecations):
- Audio analysis: `handlers/analysis.rs`, `ui/audio_analysis.rs`, `GetAudioAnalysis`, `ActiveBlock::Analysis`, `RouteId::Analysis`
- Recommendations: `GetRecommendationsForSeed`, `GetRecommendationsForTrackId`, `RouteId::Recommendations`, `TrackTableContext::RecommendedTracks`
- Made For You: `handlers/made_for_you.rs`, `GetMadeForYouPlaylistTracks`, `MadeForYouSearchAndAdd`, `ActiveBlock::MadeForYou`, `RouteId::MadeForYou`, `TrackTableContext::MadeForYou`
- Related-artists panel inside `handlers/artist.rs` (artist page itself stays)
- Featured playlists (never was wired in spotify-tui but don't add)

`LIBRARY_OPTIONS` drops from 6 to 5 entries: "Recently Played", "Liked Songs", "Albums", "Artists", "Podcasts".

## 4. `IoEvent` contract (first cut)

Model is 1 variant per Spotify call, same as spotify-tui. Full list to port (audio analysis removed):

Playback: `GetCurrentPlayback`, `RefreshAuthentication`, `StartPlayback`, `PausePlayback`, `NextTrack`, `PreviousTrack`, `Seek`, `ChangeVolume`, `Shuffle`, `Repeat`, `TransferPlaybackToDevice`, `GetDevices`, `AddItemToQueue`.

Library: `GetPlaylists`, `GetPlaylistTracks`, `GetCurrentSavedTracks`, `GetCurrentUserSavedAlbums`, `GetCurrentUserSavedShows`, `GetFollowedArtists`, `GetRecentlyPlayed`.

Browse: `GetSearchResults`, `GetArtist`, `GetAlbum`, `GetAlbumTracks`, `GetAlbumForTrack`, `GetShow`, `GetShowEpisodes`, `GetCurrentShowEpisodes`, `GetUser`.

Mutations: `ToggleSaveTrack`, `CurrentUserSavedTracksContains`, `CurrentUserSavedAlbumAdd`/`Delete`/`Contains`, `CurrentUserSavedShowAdd`/`Delete`/`Contains`, `UserFollowArtists`/`Unfollow`/`Check`, `UserFollowPlaylist`/`Unfollow`.

UI helpers: `SetTracksToTable`, `SetArtistsToTable`, `UpdateSearchLimits`.

`rspotify` 0.16 renamed many methods and restructured auth — expect ~70% of call sites to need adjustment, but the shape of each handler stays the same.

## 5. Auth flow

`rspotify` 0.16 supports **PKCE** natively. Switch to PKCE by default:

- No client secret required for user-flow auth.
- Token cache at `${XDG_CACHE_HOME:-~/.cache}/spotuify/token.json`.
- Redirect: loopback HTTP server on configured port, HTML stays in `src/auth/redirect.html`, same trick as before.
- Fallback: manual paste if port can't bind.
- Refresh: network task checks expiry before each call and refreshes inline (rspotify 0.16 has a `Token::is_expired` helper).

Scopes: same 14 as spotify-tui. Pulled out into a `const SCOPES: &[&str]` for clarity.

## 6. UI migration (`tui` 0.16 → `ratatui` 0.30)

Most of the ratatui API is a direct port. Common renames:

- `tui::Terminal` → `ratatui::Terminal` (same API)
- `Frame::size()` → `Frame::area()`
- `Block::title(Span::...)` → `Block::new().title(...)`
- `Text::styled` / `Span` / `Line` instead of `Spans`
- `List`, `Table`, `Gauge`, `Paragraph` — same shapes, constructor tweaks
- State types (`TableState`, `ListState`) unchanged in spirit

New niceties worth adopting:
- `ratatui::widgets::Scrollbar` for tables/lists
- `ratatui::symbols::border::*` for nicer frames
- `Layout::flex(Flex::Start)` replaces manual padding hacks

Responsive rules unchanged: `SMALL_TERMINAL_WIDTH=150`, `SMALL_TERMINAL_HEIGHT=45`, `BASIC_VIEW_HEIGHT=8`. When below height threshold → `BasicView`. When below width → collapse sidebar.

## 7. Config

Two files. Paths follow XDG on Linux/macOS, `%APPDATA%` on Windows, via the `directories` crate:

- `$CONFIG_DIR/spotuify/client.yml` — `{ client_id, client_secret?, redirect_port }`
- `$CONFIG_DIR/spotuify/config.yml` — `{ theme, behavior, keybindings }`

Format: **YAML** — consistent with spotify-tui so themes copy-paste across.

Keep keybindings fully remappable like spotify-tui — every handler compares to `app.config.keys.*`, never hard-codes a key. Help screen generated from that map.

## 8. Theming

Port `theme.rs`. Color fields: `active`, `banner`, `error_border`, `error_text`, `hint`, `hovered`, `inactive`, `playbar_background`, `playbar_progress`, `playbar_progress_text`, `playbar_text`, `selected`, `text`. Allow hex (`"#1db954"`) or named colors. Same field set as spotify-tui so users can paste their existing theme.

## 9. CLI mode

`clap` 4 derive. Same four top-level subcommands:

- `spot playback` — show / control current playback (`--toggle`, `--next`, `--previous`, `--volume`, `--seek`, `--shuffle`, `--repeat`, `--transfer`)
- `spot play` — play a URI / search term (`--track`, `--album`, `--artist`, `--playlist`, `--show`, `--queue`, `--device`)
- `spot list` — list devices / playlists / liked / etc. (`--format` for JSON)
- `spot search` — search + print results

CLI reuses the same `Network` + `IoEvent` plumbing; just skips spawning the UI task.

Add `--completions <shell>` and `--version` early-exit before auth, same as old app.

## 10. Milestones

Ship in vertical slices so the binary is runnable as early as possible.

**M1 — skeleton (day 1)**
- Cargo.toml with all deps pinned
- `cargo check` passes empty scaffolding
- CI (fmt + clippy + check)
- Banner + `--version` + `--help`

**M2 — auth + minimal now-playing (day 2)**
- Config loading (`client.yml`). On first run (file missing): print friendly message, open `https://developer.spotify.com/dashboard` in browser via `webbrowser::open`, prompt in terminal for Client ID + Client Secret + redirect port, write `client.yml`.
- OAuth auth-code flow + token cache at `$CONFIG_DIR/spotuify/.token_cache.json` + refresh via `rspotify` 0.16
- Single tokio runtime, three tasks (UI + Network + Events)
- `IoEvent::GetCurrentPlayback` working
- Minimal ratatui draw: playbar only, no sidebar
- `Ctrl+C` exits cleanly (restore terminal, drop raw mode, leave alt screen)

**M3 — main layout + navigation (day 3)**
- Three-pane layout, Library sidebar, MyPlaylists list, TrackTable
- Route stack push/pop, `back` key
- Block focus + hover model
- `GetPlaylists`, `GetPlaylistTracks`, `StartPlayback`, `PausePlayback`, volume, seek, next/prev

**M4 — search + browse (day 4-5)**
- Input block
- `GetSearchResults` with Albums/Artists/Tracks/Playlists/Shows tabs
- Artist page (`GetArtist` + top tracks + related)
- Album tracks
- Help menu
- Select device

**M5 — library + social (day 6-7)**
- Liked Songs, Saved Albums, Followed Artists
- Recently played
- Toggle save track/album, follow/unfollow artist
- Follow/unfollow playlist
- Delete playlist confirm dialog

**M6 — podcasts + queue (day 8)**
- Saved shows, show episodes, play episode
- Podcasts view in sidebar
- Queue view (`AddItemToQueue`, list upcoming)

**M7 — CLI mode (day 9)**
- `playback`, `play`, `list`, `search`
- `--completions`
- Integration tests for CLI output shape

**M8 — polish (day 10)**
- Theme file + a couple of preset themes
- Basic view for tiny terminals
- Responsive layout breakpoints
- Queue view
- `tracing` file log at `$CACHE_DIR/spotuify/log`
- README with screenshots

No pitches/audio-analysis ever.

## 11. Testing

- Unit tests for: config parsing, pagination state machine, key dispatch (pure function from `(KeyEvent, ActiveBlock, KeyBindings) → Action`), `ms_to_mm_ss` helper.
- Integration test for CLI: spawn `spotuify search --format json` against a mock `Spotify` trait object — exercise argument parsing and output formatting without real auth.
- No UI snapshot tests initially — too brittle. Revisit with `insta` + ratatui test backend if we keep hitting rendering regressions.

## 12. Decisions (resolved)

1. **Binary**: `spot`. Crate + config dir: `spotuify`.
2. **Config format**: YAML.
3. **First-run**: if `client.yml` missing, open Spotify developer dashboard in browser and interactively prompt for ID/secret/port in the terminal, then write the file. No manual-only path.
4. **Dropped features** (deprecated by Spotify Web API on 2024-11-27, not available to new apps): audio analysis, recommendations, related artists, featured playlists, Made For You / algorithmic playlists.
5. **Rust version**: latest stable, no MSRV pin.
