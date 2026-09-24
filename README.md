# spotuify

A terminal client for Spotify, written in Rust with [ratatui](https://ratatui.rs). Inspired by [`spotify-tui`](https://github.com/Rigellute/spotify-tui).

spotuify controls playback through the Spotify Web API. It does not play audio itself: the sound comes from any Spotify Connect device on your account (the desktop app, your phone, `spotifyd`, …).

## Features

- Browse Liked Songs, saved albums, followed artists, podcasts, recently played and your playlists
- Search tracks, albums and artists
- Play/pause, skip, seek, volume, shuffle, repeat, queue, device switching
- Save tracks and albums, follow artists
- Cover art in the terminal (needs `ffmpeg`)
- Built-in themes with a live picker, plus themes that follow the time of day or the release decade of the current track
- Every keybinding is configurable

## Requirements

- Spotify Premium
- An active Spotify Connect device
- Your own Spotify developer app (Client ID and Secret); the first-run setup walks you through it
- `ffmpeg` (optional, for cover art)

## Install

```bash
git clone https://github.com/JinHedman/spotuify
cd spotuify
cargo build --release
./target/release/spot
```

On Linux you also need `libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev pkg-config`.

## First run

If no credentials are found, `spot` starts a setup wizard:

1. Create an app at <https://developer.spotify.com/dashboard>, tick **Web API**, and set the redirect URI to `http://127.0.0.1:8888/callback`.
2. Paste the Client ID and Secret into the terminal.
3. Approve access in the browser, then paste the URL you were redirected to back into the terminal.

The token is cached and refreshed automatically after that.

## Usage

Press `?` in the app to see all keybindings. Some common ones:

| Key | Action |
|-----|--------|
| `/` | Search |
| `Enter` | Play / open |
| `Space` | Play / pause |
| `n` / `p` | Next / previous track |
| `+` / `-` | Volume |
| `[` / `]` | Seek |
| `d` | Select device |
| `Q` | Show queue |
| `A` | Add to queue |
| `t` | Theme picker |
| `Esc` | Quit |

Note that the default `j`/`k` bindings are swapped: `k` moves down and `j` moves up. Change them in the config if you prefer the usual vim layout.

## Configuration

Config lives in the `spotuify` folder of your OS config directory (`~/Library/Application Support/io.spotuify` on macOS, `~/.config/spotuify` on Linux):

- `client.yml`: Spotify credentials, written by the setup wizard
- `config.yml`: optional theme, behavior and keybinding overrides

Only include the fields you want to change:

```yaml
theme:
  active: "#1db954"
  inactive: DarkGray

behavior:
  poll_interval_ms: 3000
  volume_step: 10
  only_own_playlists: true   # hide playlists you follow but didn't create

keybindings:
  move_down: [j, Down]
  move_up: [k, Up]
```

Colors accept named terminal colors (`Red`, `DarkGray`, …) or hex (`"#1db954"`). Ready-made palettes are in [`themes/`](./themes/).

## Limitations

Spotify has restricted parts of the Web API for new apps, so spotuify cannot offer:

- Recommendations, related artists, audio features, or algorithmic playlists (Discover Weekly etc.)
- Track listings for playlists you don't own (they still play)
- Playlist folders or local files

## License

MIT
