# spotuify

A modern Rust terminal client for Spotify. Inspired by [`spotify-tui`](https://github.com/Rigellute/spotify-tui), rebuilt on current crates:

- `ratatui` 0.30 (replaces the unmaintained `tui-rs`)
- `crossterm` 0.29 (event-stream backend)
- `rspotify` 0.16
- `tokio` 1 (single runtime, three tasks)
- `clap` 4
- Rust edition 2021

**Binary:** `spot`. Config directory: `$CONFIG_DIR/spotuify/`.

Like `spotify-tui`, this only *controls* playback via the Spotify Web API. Audio playback itself comes from the official Spotify desktop app, `spotifyd`, or any other Spotify Connect-enabled client running on the same account.

## Requirements

- **Spotify Premium** — Connect playback control is Premium-only.
- **An active Spotify Connect device** on your account (desktop app, phone, `spotifyd`, etc.). `spot` hands off to an existing device; it doesn't produce audio.
- **ffmpeg** (optional) — only needed for playlist cover art. Without it the cover pane shows `(ffmpeg not found)` and everything else works normally. `brew install ffmpeg` / `apt install ffmpeg`.
- **Your own Spotify developer app** — every user needs their own Client ID / Secret. Spotify does not allow sharing credentials. The first-run wizard walks you through this.

## Install / build

```bash
cd spotuify
cargo build --release
./target/release/spot
```

On Linux, clipboard support needs `libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev` and the usual `libssl-dev pkg-config`.

## First run

If `$CONFIG_DIR/spotuify/client.yml` is missing, `spot` walks you through setup:

1. Opens <https://developer.spotify.com/dashboard> in your browser.
2. Asks you to:
   - Click **Create app**
   - Fill in any name + description
   - Set the redirect URI to **`http://127.0.0.1:8888/callback`** (exactly)
   - Tick **Web API**
   - Save, then copy the Client ID and Client Secret
3. Prompts for Client ID, Client Secret, and (optional) redirect port.
4. Opens the Spotify auth URL; you approve; browser is redirected to `http://127.0.0.1:8888/callback?code=...`.
5. You copy that full URL from the browser address bar and paste it back into the terminal.
6. The TUI launches.

Subsequent runs use the cached token at `$CONFIG_DIR/spotuify/.token_cache.json` and refresh it automatically.

## Keybindings

Press `?` at any time for the in-app help overlay. It is generated from your actual bindings, so it stays correct after a rebind — as does the status-line legend. Defaults:

| Key | Action |
|-----|--------|
| `?` | Toggle help overlay |
| `Esc` / `Ctrl+C` | Quit |
| `Ctrl+h` / `Ctrl+l` | Focus pane left / right |
| `Ctrl+j` / `Ctrl+k` | Focus pane up / down (inverted) |
| `k` / `j` or `↓` / `↑` | Move selection within list (inverted j/k) |
| `K` / `J` | Move selection by 5 (inverted) |
| `g` / `G` | Top / bottom of list |
| `Enter` / `l` | Activate / open selected item |
| `h` / `q` / `b` / `Backspace` | Back (pop navigation) |
| `/` | Open search input |
| `Tab` / `Shift+Tab` | Cycle tabs in current view (e.g. search results) |
| `←` / `→` | Cycle tabs (alt) |
| `Space` | Play / pause |
| `n` / `p` | Next / previous track |
| `+` / `-` | Volume ±10 |
| `[` / `]` | Seek ±5 seconds |
| `r` | Refresh current playback |
| `z` | Toggle shuffle |
| `R` | Cycle repeat (off → all → one) |
| `s` | Save / unsave current track |
| `S` | Save / unsave current album |
| `f` | Follow / unfollow current artist |
| `d` | Select playback device |
| `Q` | Show playback queue |
| `A` | Add selected track / episode to queue |
| `D` | Remove highlighted playlist from your library (confirm) |
| `t` | Open theme picker (arrow keys to preview, Enter to apply, Esc to cancel) |

All keys except `Ctrl+C` are remappable via `config.yml` (see below).

## Features

### Works today

- **Authentication** — OAuth auth-code flow with cached token + automatic refresh.
- **Playbar** — three rows beside an 8x3 cover thumbnail: the track identity on one line, elapsed-total with shuffle/repeat/volume centred beneath it, and a progress bar spanning the full width the cover leaves free. Costs one row from the content area above compared with the old two-row layout. The thumbnail is too small to recognise a cover but carries the album's colour; it stays silent when there is no art rather than showing a placeholder.
- **Sidebar** — `Library` (fixed entries) + `Playlists` (your own playlists) + `Cover` art for the selected playlist.
- **Playlist cover art** — the selected playlist's cover is drawn with half-block glyphs (two full-colour pixels per terminal cell, 24x24 px). Decoding is shelled out to ffmpeg, which fetches the image URL itself; no extra Spotify request is made. Needs enough sidebar height, otherwise the pane is dropped.
- **Cover cache** — rendered covers are cached in memory for the session and on disk across runs, so ffmpeg runs once per distinct artwork rather than on every selection or restart. Cache lives in the OS cache directory (`~/Library/Caches/io.spotuify/covers` on macOS, `~/.cache/spotuify/covers` on Linux); each entry is under 2 KB. Entries are keyed by artwork *and grid size*, so the sidebar's 24x12 and the playbar's 8x3 renders of the same album are separate entries and neither can be served for the other. Changing a playlist's picture invalidates it automatically. Safe to delete at any time — it will simply be rebuilt.
- **Owner filter** — by default only playlists you created are listed, because Spotify 403s track listings for anyone else's playlist (see "Known limitations"). The `Playlists` title reports how many were hidden. Set `behavior.only_own_playlists: false` to show them all again; `Enter` still starts playback on a followed playlist even though its listing fails.
- **Library entries**
  - **Liked Songs** → saved tracks in TrackTable
  - **Albums** → saved-album list → Enter loads album tracks
  - **Artists** → followed-artist list → Enter loads top tracks *(see caveat below)*
  - **Podcasts** → saved-show list → Enter loads episodes → Enter plays
  - **Recently Played** → recently played tracks (deduped)
- **TrackTable** — columns for #, Title, Artist, Album, Time. `Enter` plays from selected position. Supports playing from playlist/album contexts or standalone URI lists.
- **Search** — `/` to open. Submits to Spotify search with 4 tabs (Tracks / Albums / Artists / Playlists). Enter on a track plays; Enter on album/artist/playlist opens it in TrackTable.
- **Queue** — `Q` shows currently playing + upcoming. `A` from TrackTable / ShowEpisodes / search-track-results adds the selected item to the queue.
- **Device selector** — `d` lists your Spotify Connect devices. `Enter` transfers playback.
- **Playback control** — play/pause, next/prev, seek, volume, shuffle (`z`), repeat (`R`).
- **Shuffle / repeat indicators** — shown on the playbar timeline row: `⇄` for shuffle, `↻` for repeat-all, `↻1` for repeat-one. Dim when off. Worth knowing that `n` / `p` follow Spotify's queue, so with shuffle on they jump to a random track — the indicator is there to make that visible rather than surprising.
- **Library glyphs** — the sidebar entries carry markers: `♥` Liked Songs, `◎` Albums, `★` Artists, `◉` Podcasts, `◷` Recently Played. All single-width and none reusing a glyph that already means something else (`▶` selection, `⏸` play state, `↻` repeat, `⇄` shuffle).
- **Scrollbars** — the track table and playlist list show a position indicator, but only when the list overflows: on a short list its absence is the signal that there is nothing more to see.
- **Volume flash** — the volume figure lights up and settles back when the level changes, including changes made from another Spotify client.
- **Now-playing marker** — the row holding the current track or episode is tinted and its row number replaced by a three-bar equalizer, animated while playing and flat when paused. Shown in the track table and the episode list. Matched by URI, so a track appearing twice in a playlist marks both rows — Spotify reports what is playing but not where in the context it sits, so this is not resolvable.
- **Smooth progress bar** — sub-cell resolution via unicode eighth-blocks, so the extrapolated progress actually shows as smooth motion.
- **Save toggle** — `s` toggles save/unsave for the currently-playing track, `S` for the current album, `f` for the current artist.
- **Remove playlist** — `D` on a highlighted playlist opens a confirmation dialog; confirming unfollows it (which deletes your own playlists).
- **Block navigation** — every view is a block; `Tab` cycles, Enter pushes onto a history stack, `b`/`Backspace` pops.
- **Scrolling** — vim-style with a 2-row scrolloff margin: the view starts sliding 2 rows before the selection hits the top or bottom of the visible area, symmetric up and down.
- **Help overlay, status line** — the bottom row shows the key legend, and borrows it for a few seconds when there is something to report. Messages expire on their own; they no longer replace the playbar, so an error never costs you the track, cover, progress or controls.
- **Responsive layout** — sidebar auto-collapses below ~110 columns; basic playbar-only view when the terminal is very short; a "terminal too small" placeholder below 60×10.
- **Configurable theme + keybindings** via `$CONFIG_DIR/spotuify/config.yml` (see "Config" below). A few ready-made palettes live in [`themes/`](./themes/).
- **Era theme** — a second decade set, drawn from documented period colour rather than from how each decade reads on screen today: **Era**, spanning eight palettes from the 1950s to the 2020s. It starts a decade earlier than the Decade set, so pre-1960 recordings get their own palette instead of clamping up. Sourced from [Onyx Creative's palettes by decade](https://www.onyxcreative.com/blog/2020/9/popular-color-palettes-by-decade); where the source names only colours too dark to serve as a UI accent (the 1990s is plum, brown and black) the hue is kept and lifted for `active`, with the true darks taking the background roles. Anchors live in [`themes/eras/`](./themes/eras/).
- **Pinning one palette** — the picker lists the two decade sets as single entries rather than every palette inside them, so the individual palettes are data the auto modes resolve against rather than alternatives to scroll past. To stay on one permanently, copy the contents of the file you want (e.g. [`themes/eras/1980s.yml`](./themes/eras/1980s.yml)) into your `config.yml`.
- **Time-of-day theme** — pick **Time of day** in the theme picker (`t`) and the palette travels through the day: small hours, dawn, morning, midday, afternoon, dusk, night. It interpolates continuously between the seven anchor palettes rather than stepping between them, so the colour drifts as the hours pass instead of jumping at boundaries. The picker shows the current phase. Anchors live in [`themes/timeofday/`](./themes/timeofday/).
- **After-dark warmth** — toggle it in the theme picker (`t`) with `Space` on the **After dark** row: it shows as a checkbox rather than a swatch, because it layers on top of whichever theme you have selected instead of replacing it. The choice persists to `$CONFIG_DIR/spotuify/.time_of_day_shift`, which overrides `config.yml` the same way `.selected_theme` does. Also settable directly as `behavior.time_of_day_shift` (0.0 off, 1.0 full). It warms and dims the palette as the evening draws on: neutral 09:00–17:00, ramping to full by 23:00, holding until 05:00, then ramping back. It is a *modifier*, not a theme, so it layers on top of whatever you have selected — including decade mode. `error` is never warmed, since blunting the one colour whose job is to alarm you would defeat it. Named colours pass through untouched for the same reason fades skip them: their RGB belongs to your terminal.
- **Decade themes** — pick **Decade** in the theme picker (`t`) and it re-themes the UI to match the release decade of whatever is playing, fading between them. The picker shows the decade it currently resolves to. Tracks with no usable release date fall back to Spotify Green, so an unknown-year track looks the same whichever theme you switched from; years outside the table clamp to the nearest end, so a 1955 recording gets the 1960s palette.
- **Theme fades** — theme changes blend over `behavior.theme_transition_ms` (350ms default, 0 to disable) instead of snapping, and the UI redraws at ~30fps for the duration so the blend reads as motion rather than two steps. Only blends between hex colours: named colours like `DarkGray` belong to your terminal palette, so interpolating them would substitute a guess for your configured colour — those snap at the midpoint instead.
- **Live theme picker** — press `t` to preview built-in presets on the fly (Spotify Green, Gruvbox Dark, Solarized Dark, Nord, Monokai, Catppuccin Mocha, Tokyo Night, Rosé Pine, Dracula, Everforest Dark, Kanagawa). Enter applies and persists (writes `$CONFIG_DIR/spotuify/.selected_theme`, which is re-applied on next launch). Esc reverts. Delete `.selected_theme` to go back to your `config.yml` theme, or copy the matching `themes/*.yml` into `config.yml` for a config-tracked change.

### Known limitations (not our fault)

Spotify deprecated a large slice of the Web API for new apps on **2024-11-27**. For apps that didn't already have extended-mode access, these endpoints return 403/404:

- `GET /recommendations` — Recommendations
- `GET /artists/{id}/related-artists` — Related artists
- `GET /audio-features`, `GET /audio-analysis` — Audio features / analysis
- `GET /browse/featured-playlists` — Featured playlists
- Algorithmic playlists (Discover Weekly, Release Radar, Daily Mix) — no longer returned
- `GET /artists/{id}/top-tracks` — Top tracks (flagged deprecated in rspotify 0.16; the call is kept under `#[allow(deprecated)]`, works only for accounts with legacy extended-mode access)

Spotuify does not implement any of these. They are called out in [`PLAN.md`](./PLAN.md) §3.

Other Spotify-side limitations:

- **`GET /playlists/{id}/items` returns 403** for apps without extended quota when the playlist isn't yours — so tracks of followed playlists can't be listed. Playback still works via Spotify Connect. This is why `behavior.only_own_playlists` defaults to `true`.
- **Playlist folders** are not exposed by the Web API at all — they are a local-client feature. The playlist list is flat. This is a ~10-year standing request to Spotify.
- **Local tracks** (imported MP3s) have no URI and can't be played via the Web API.

## Config

Two files live under `$CONFIG_DIR/spotuify/`:

### `client.yml`

Your Spotify app credentials. Created automatically by the first-run wizard:

```yaml
client_id: "…"
client_secret: "…"
redirect_port: 8888
```

### `config.yml` (optional)

Controls theme, behavior, and keybindings. Missing file = built-in defaults. A malformed file does **not** stop the app — it falls back to defaults and reports the parse error, with line and column, in the playbar. Every field has a sensible default; you only need to include the fields you want to override.

```yaml
theme:
  active:        "#1db954"   # focused block borders, tab highlight, play icon
  inactive:      DarkGray     # unfocused block borders
  selected_bg:   DarkGray     # highlighted row background
  hint:          DarkGray     # dim text (subtitle, legend labels, hints)
  error:         Red          # error messages in the playbar
  progress:      "#1db954"    # progress bar fill
  playing_icon:  "#1db954"    # ▶ / ⏸ icon

behavior:
  poll_interval_ms: 3000      # how often to re-poll current playback
  tick_rate_ms: 200           # UI redraw tick (governs progress-bar smoothness)
  volume_step: 10             # increment for +/-
  seek_step_ms: 5000          # increment for [/]
  only_own_playlists: true    # hide playlists you follow but didn't create
  theme_transition_ms: 350    # theme fade duration; 0 to snap instantly
  time_of_day_shift: 0.0      # after-dark warm/dim, 0.0 off … 1.0 full
                              # (the theme picker overrides this; delete
                              #  .time_of_day_shift to fall back to it)

keybindings:
  # Each action accepts a single key or a list of keys.
  # Names: Space, Tab, BackTab, Esc, Enter, Backspace, Up, Down, Left, Right,
  # PageUp, PageDown, Home, End, Delete, or any single printable character.
  # Modifier prefixes: ctrl+, alt+, shift+.
  quit:             Esc
  back:             [b, Backspace, q, h]
  activate:         [Enter, l]
  help:             "?"
  search:           "/"
  device:           d
  queue:            Q
  refresh:          r
  play_pause:       Space
  next_track:       n
  previous_track:   p
  volume_up:        ["+", "="]
  volume_down:      ["-", "_"]
  seek_forward:     "]"
  seek_backward:    "["
  save_track:       s
  save_album:       S
  follow_artist:    f
  delete_playlist:  D
  theme_picker:     t
  shuffle:          z
  repeat:           R
  add_to_queue:     A
  block_left:       ctrl+h
  block_right:      ctrl+l
  block_up:         ctrl+j
  block_down:       ctrl+k
  move_down:        [k, Down]
  move_up:          [j, Up]
  move_down_big:    K
  move_up_big:      J
  move_top:         g
  move_bottom:      G
  search_tab_next:  [Tab, Right]
  search_tab_prev:  [BackTab, Left]
```

Color values accept any of:
- Named colors: `Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`, `Gray`, `DarkGray`, `LightRed`, `LightGreen`, `LightYellow`, `LightBlue`, `LightMagenta`, `LightCyan`, `Reset` (terminal default)
- Hex RGB: `"#1db954"`

`Ctrl+C` is hard-wired to quit and cannot be remapped.

## Architecture (short version)

Single Tokio runtime. Three concurrent actors:

1. **UI task** (main) — owns the terminal, runs the draw/event loop, dispatches keys to handlers.
2. **Network task** — `tokio::spawn`'d; consumes `IoEvent` messages from an `mpsc::channel` and calls `rspotify`. Writes results back into the shared `Arc<Mutex<AppState>>`.
3. **Event task** — implicit in `crossterm::event::EventStream` consumed by the UI task inside a `tokio::select!`.

The `IoEvent` enum in [`src/client/mod.rs`](./src/client/mod.rs) is the full contract between UI and network — every Spotify call is one variant. Adding a new Spotify interaction means: add an `IoEvent` variant, handle it in `Network::dispatch`, and dispatch it from a handler via `io_tx.send`.

Full design notes are in [`PLAN.md`](./PLAN.md).

## Roadmap

### Intentionally skipped

- **CLI mode** (`spot playback`, `spot play`, `spot list`, `spot search`) — was planned for M7 but dropped.
- **Tracing file log** — dropped.
- **README screenshots** — dropped.

### Social actions (still deferred)

- Follow a playlist from a search result (not yet wired — only "remove from library" via `D` on the sidebar is available today).


### If Spotify ever re-enables them

- Recommendations view (seed from artist / seed from track)
- Related-artists panel on artist pages
- Audio analysis / pitches view (the one we explicitly dropped to avoid visual clutter anyway)
- Made For You / Discover Weekly / Release Radar

## License

MIT.
