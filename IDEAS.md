# Feature ideas

Backlog of candidate features, with the design work and known gotchas captured
while they were still fresh. Nothing here is committed to — see `PLAN.md` for
decisions actually taken, and `README.md` for what exists today.

Every idea here has been checked against the Spotify API constraints in
`CLAUDE.md`. Ideas that turned out to be impossible are recorded at the bottom
so they don't get re-proposed.

---

## 1. Album art for the now-playing track

Extend the cover pipeline from playlists to whatever is playing.

**Why it's cheap:** no new machinery at all. `render_cover`, `cover_cache_key`
and both cache tiers already exist and are keyed by artwork, so a track's album
art hits the same cache as a playlist cover showing the same image.

**Where the image comes from:** `AppState.playback` →
`CurrentPlaybackContext.item` → `PlayableItem::Track(FullTrack)` →
`track.album.images`. Same `smallest_image_url` selection as playlists.

**Placement.** The playbar is `Constraint::Length(4)`, so an inline cover there
is 8x4 cells — established as an unrecognisable blob during the half-block
evaluation. Real options:

- **Full-screen cover mode** on a keypress, as a new `ActiveBlock`. At ~60
  columns this is 60x60 px, which is the first size where a cover is properly
  legible rather than merely suggestive. Best visual payoff.
- **Reuse the sidebar cover pane**, showing now-playing when the Playlists
  block isn't focused and the selected playlist's art when it is. No new
  layout, but no size improvement either.

Doing both is reasonable: the sidebar pane becomes context-sensitive, and the
full-screen mode is there when you actually want to look at it.

**Gotchas**

- Don't render on every playback poll. The poll runs every
  `poll_interval_ms` (3s default); render only when the track id changes,
  otherwise ffmpeg spawns twenty times a minute.
- Episodes have no album. `PlayableItem::Episode` carries images on the show,
  so that branch needs handling or explicit skipping.
- Nothing is playing is a normal state, not an error — the pane needs a
  resting appearance.

---

## 2. Theme the UI from the album art

Drive the accent colour from what's playing, so the whole interface shifts
colour with the music.

**Why it's cheap:** the raw RGB is already in `CoverArt.cells` after a render.
No fetch, no new dependency, no ffmpeg invocation beyond the one already
happening. `state.theme` is already runtime-mutable — the theme picker mutates
it today — so the plumbing exists.

**Extracting the colour.** Averaging the whole image gives mud; this is the
part that needs actual care. Workable approach: quantise pixels into coarse
buckets (4 bits per channel), discard buckets that are near-black, near-white
or very desaturated, then take the most populous survivor. Falls back to the
configured theme colour when nothing survives — which is common, since a lot
of covers are mostly monochrome.

**Contrast is the real problem, not extraction.** `theme.active` paints focused
borders, the tab highlight and the play icon; `theme.progress` fills the
progress bar. A dark accent pulled from a dark cover is invisible on a dark
terminal. The extracted colour must be clamped into a usable luminance band
(adjust lightness in HSL, keep the hue) rather than used raw. Getting this
wrong makes the app unusable rather than merely ugly, so it deserves tests over
a spread of real covers.

**Scope the override deliberately.** Only `active`, `progress` and
`playing_icon` should follow the art. Leave `error` alone — errors must stay
red — and leave `selected_bg` and `hint` alone so text contrast stays
predictable.

**Gotchas**

- Must not persist. The theme picker writes `.selected_theme`; this must never
  touch that file, or a colour derived from one track becomes the permanent
  theme.
- Precedence against the theme picker needs deciding. Pressing `t` while
  art-theming is active is currently ambiguous — probably art-theming yields
  until playback changes, or is suspended while the picker is open.
- Needs reverting when playback stops, otherwise the UI keeps the colour of
  whatever played last.
- Gate behind `behavior.theme_from_album_art`, following the
  `only_own_playlists` precedent. Opinionated enough that it should be
  switchable without a rebuild.

---

## 3. Jump to the current context

One key to navigate to whatever the current track is playing from — the
playlist, album or artist — instead of hunting for it by hand.

**Source:** `AppState.playback` → `CurrentPlaybackContext.context` →
`Context.uri`, e.g. `spotify:playlist:37i9dQ...`. Parse the type and id out of
the URI and dispatch the `IoEvent` that already exists for it:
`GetPlaylistTracks`, `GetAlbumTracks` or `OpenArtist`.

**The name problem.** Those events take a display name for the pane title, and
the playback response doesn't carry the context's name. Resolution differs per
type:

- **Album** — available from the playing track itself
  (`track.album.name`), no fetch needed.
- **Playlist** — not in the playback response. Look it up in
  `state.playlists` first, which will usually hit now that the owner filter
  means the list is your own playlists. Fall back to a `/playlists/{id}`
  fetch only on a miss.
- **Artist** — available from `track.artists`.

**Gotchas**

- `context` is `Option` and legitimately absent. Playing a bare URI list via
  `PlayTrackUris` sets no context, and neither do local files or autoplay.
  The key should do nothing quietly, not error.
- Liked Songs appears as a `collection` URI (`spotify:user:<id>:collection`)
  with no usable id — special-case it to the Liked Songs view rather than
  trying to parse an id out of it.
- Needs a new `config.keys` entry. Never hardcode the key.

---

## 4. Search history

Recall previous queries in the search input rather than retyping them.

**State:** a capped `Vec<String>` on `AppState` plus a cursor for cycling.
Pushing a query that's already present should move it to the front rather than
duplicating it.

**Keys are already free.** `handlers/input.rs` matches `KeyCode::Char(c)` to
append text and falls through `_ => {}` for everything else, so `Up`/`Down` are
unused in the input and can cycle history without touching normal typing. This
matters because `move_up`/`move_down` are bound to `[j, Up]` / `[k, Down]` —
inside a text field `j` and `k` must insert characters, so the arrow keys are
the only usable binding for this. Reusing the configured list keys here would
break typing.

**Persistence:** session-only is the simple version. A capped file under the
cache dir would survive restarts, alongside the cover cache; the same
"derived data, safe to delete" reasoning applies.

**Gotchas**

- Cycling must leave `search_query` in a sane state when the user walks past
  the end of the history — restoring the partially typed query, ideally.
- Search results are capped at 10 by Spotify's Feb 2026 change, so repeating
  searches is common and this earns its keep.

---

## Known impossible

Recorded so they don't get re-proposed. See `CLAUDE.md` for the full
deprecation list.

- **Any real audio visualiser** — waveform, spectrum, beat detection. spotuify
  never touches the audio stream; playback happens on the Connect device.
  Anything animated would have to be faked from `progress_ms`, which reads as
  a gimmick as soon as you know it isn't real.
- **Lyrics** — not exposed by the Web API at all. Would need a third-party
  service and an API key.
- **Queue reordering** — the queue API is read-plus-append only. Playlist
  reordering *is* supported; the queue isn't.
