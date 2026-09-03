pub mod route;

pub use route::{ActiveBlock, ArtistTab, SearchTab};

use crate::config::theme::Theme;
use crate::config::user::UserConfig;
use rspotify::model::{
  album::{SavedAlbum, SimplifiedAlbum},
  artist::FullArtist,
  context::CurrentPlaybackContext,
  device::Device,
  playlist::SimplifiedPlaylist,
  show::{Show, SimplifiedEpisode},
  track::{FullTrack, SimplifiedTrack},
  PlayableItem,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A fixed sidebar entry: its glyph and its name.
///
/// One array rather than a name list plus a parallel icon list, so the two
/// cannot drift out of order. The handler matches on `name`, never on what is
/// rendered, so adding or changing a glyph can't break navigation.
pub struct LibraryEntry {
  /// Single-width glyph. Chosen from Geometric Shapes and Miscellaneous
  /// Symbols, which terminal fonts render at one cell — an emoji-presented
  /// glyph would take two and shift the whole sidebar.
  ///
  /// None of these collide with a glyph that already means something else:
  /// `▶` is selection, `⏸` play state, `↻` repeat, `⇄` shuffle.
  pub icon: &'static str,
  pub name: &'static str,
}

pub const LIBRARY_ENTRIES: [LibraryEntry; 5] = [
  LibraryEntry {
    icon: "\u{2665}", // ♥ liked
    name: "Liked Songs",
  },
  LibraryEntry {
    icon: "\u{25ce}", // ◎ disc
    name: "Albums",
  },
  LibraryEntry {
    icon: "\u{2605}", // ★ performers
    name: "Artists",
  },
  LibraryEntry {
    icon: "\u{25c9}", // ◉ on air
    name: "Podcasts",
  },
  LibraryEntry {
    icon: "\u{25f7}", // ◷ elapsed
    name: "Recently Played",
  },
];

/// Everything a preview has to restore on cancel.
///
/// `theme` alone is not enough: `apply_theme_source` recomputes the target
/// from the mode and fixed palette every frame, so a cancel that restored
/// only the rendered theme would be reverted on the very next frame.
#[derive(Debug, Clone, Copy)]
pub struct ThemeSnapshot {
  theme: Theme,
  mode: ThemeMode,
  fixed: Theme,
}

/// Where the active theme comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
  /// A single palette, chosen by the user.
  Fixed,
  /// Follows the release decade of whatever is playing.
  DecadeAuto,
  /// Follows the release decade using the era palettes.
  EraAuto,
  /// Follows the clock, drifting through the day's palettes.
  TimeOfDayAuto,
}

/// A fade from one theme to another.
///
/// Every theme change routes through `AppState::set_theme`, so sources added
/// later — album art, decade palettes, time-of-day — inherit fading without
/// touching this.
#[derive(Debug, Clone, Copy)]
pub struct ThemeTransition {
  from: Theme,
  to: Theme,
  started: Instant,
  duration: Duration,
}

/// Cover art is drawn with the upper-half-block glyph: each terminal cell
/// carries two independently coloured pixels (foreground = top, background =
/// bottom), so a cell grid of `COVER_COLS x COVER_ROWS` renders
/// `COVER_COLS x (COVER_ROWS * 2)` full-colour pixels.
///
/// 24x12 cells fits the 36 usable columns of the sidebar and costs 12 rows.
/// Terminal cells are roughly 2:1, so this is square on screen.
pub const COVER_COLS: u16 = 24;
pub const COVER_ROWS: u16 = 12;

/// Now-playing thumbnail in the playbar. 8x3 cells is 8x6 pixels — far too
/// small to recognise a cover, which the half-block evaluation established,
/// but enough to carry the album's colour beside the title.
pub const NOWPLAYING_COLS: u16 = 8;
pub const NOWPLAYING_ROWS: u16 = 3;

/// One rendered cover: `COVER_ROWS * COVER_COLS` cells, row-major, each a
/// (top, bottom) RGB pair.
#[derive(Clone, Debug)]
pub struct CoverArt {
  pub cols: u16,
  pub rows: u16,
  pub cells: Vec<(Rgb, Rgb)>,
}

pub type Rgb = (u8, u8, u8);

/// A resolved cover and what it belongs to.
///
/// `art: None` means we looked and Spotify had no image — distinct from the
/// whole field being None, which means we have not looked yet. Keeping them
/// apart stops the network task from re-spawning ffmpeg for something that
/// will never have art.
///
/// `id` is a playlist id for the sidebar cover and a track or episode URI for
/// the playbar one; both are compared only against themselves.
#[derive(Clone, Debug)]
pub struct CachedCover {
  pub id: String,
  pub art: Option<CoverArt>,
}

#[derive(Clone, Debug)]
pub struct TrackRow {
  pub uri: Option<String>,
  pub name: String,
  pub artists: String,
  pub album: String,
  pub duration_ms: u64,
}

impl TrackRow {
  pub fn from_full(t: FullTrack) -> Self {
    Self {
      uri: t.id.as_ref().map(|id| {
        use rspotify::prelude::Id;
        id.uri()
      }),
      name: t.name,
      artists: join_artist_names(t.artists.iter().map(|a| a.name.as_str())),
      album: t.album.name,
      duration_ms: t.duration.num_milliseconds().max(0) as u64,
    }
  }

  pub fn from_simplified(t: SimplifiedTrack, album_name: &str) -> Self {
    Self {
      uri: t.id.as_ref().map(|id| {
        use rspotify::prelude::Id;
        id.uri()
      }),
      name: t.name,
      artists: join_artist_names(t.artists.iter().map(|a| a.name.as_str())),
      album: album_name.to_string(),
      duration_ms: t.duration.num_milliseconds().max(0) as u64,
    }
  }
}

/// Year from a Spotify release date, which may be "2020", "2020-05" or
/// "2020-05-12" depending on `release_date_precision`.
///
/// `str::get` returns None on a non-char-boundary rather than panicking, so a
/// malformed value is simply rejected. The range check discards nonsense that
/// would otherwise clamp to an end palette and look deliberate.
fn parse_release_year(raw: &str) -> Option<u16> {
  let year: u16 = raw.get(..4)?.parse().ok()?;
  (1000..=2999).contains(&year).then_some(year)
}

/// Narrowest image in the list. Spotify orders them largest-first, but
/// `width` is nullable, so an unknown width must not win the minimum.
fn smallest_image(images: &[rspotify::model::Image]) -> Option<String> {
  images
    .iter()
    .min_by_key(|i| i.width.unwrap_or(u32::MAX))
    .or_else(|| images.last())
    .map(|i| i.url.clone())
}

fn join_artist_names<'a, I: Iterator<Item = &'a str>>(iter: I) -> String {
  iter.collect::<Vec<_>>().join(", ")
}

#[derive(Default)]
pub struct SearchResults {
  pub tracks: Vec<FullTrack>,
  pub albums: Vec<SimplifiedAlbum>,
  pub artists: Vec<FullArtist>,
  pub tracks_index: usize,
  pub albums_index: usize,
  pub artists_index: usize,
}

pub struct ArtistView {
  pub artist_id: String,
  pub artist_name: String,
  pub tracks: Vec<TrackRow>,
  pub tracks_index: usize,
  pub tracks_offset: usize,
  pub albums: Vec<SimplifiedAlbum>,
  pub albums_index: usize,
  pub albums_offset: usize,
  pub tab: ArtistTab,
}

impl Default for ArtistView {
  fn default() -> Self {
    Self {
      artist_id: String::new(),
      artist_name: String::new(),
      tracks: Vec::new(),
      tracks_index: 0,
      tracks_offset: 0,
      albums: Vec::new(),
      albums_index: 0,
      albums_offset: 0,
      tab: ArtistTab::Tracks,
    }
  }
}

pub struct AppState {
  pub config: Arc<UserConfig>,

  /// Live theme — initialized from config, can be changed at runtime via the
  /// theme picker. UI code should read from `state.theme`, not `state.config.theme`.
  pub theme: Theme,
  /// Saved theme source so the picker can revert on cancel.
  pub theme_before_preview: Option<ThemeSnapshot>,
  /// Which source decides the theme.
  pub theme_mode: ThemeMode,
  /// The chosen fixed palette. Used directly in `Fixed` mode, and as the
  /// fallback in `DecadeAuto` when a track's year is unknown.
  pub theme_fixed: Theme,
  /// Last volume we observed, and when it changed — drives the playbar
  /// flash. Tracked from observed state rather than from the keypress, so a
  /// change made on another Spotify client flashes too.
  pub last_volume: Option<u32>,
  pub volume_changed_at: Option<Instant>,
  /// After-dark modifier strength, 0.0 off to 1.0 full. Starts from
  /// `config.behavior.time_of_day_shift` and can be toggled at runtime.
  pub time_of_day_shift: f32,
  /// In-flight fade, if any. `theme` above is the *rendered* result; this is
  /// what it is moving toward.
  pub theme_transition: Option<ThemeTransition>,
  pub theme_picker_index: usize,

  pub playback: Option<CurrentPlaybackContext>,
  pub playback_received_at: Option<Instant>,
  pub last_error: Option<String>,
  pub is_loading: bool,

  pub active_block: ActiveBlock,
  pub block_history: Vec<ActiveBlock>,

  pub library_index: usize,

  pub playlists: Vec<SimplifiedPlaylist>,
  pub playlists_index: usize,
  pub playlists_offset: usize,
  /// How many playlists the owner filter removed, so the UI can say so
  /// instead of silently showing a short list.
  pub playlists_hidden: usize,
  /// Rendered cover for the currently selected playlist, if any.
  pub playlist_cover: Option<CachedCover>,
  /// Cover for whatever is playing, keyed by its URI.
  pub now_playing_cover: Option<CachedCover>,
  /// Covers already rendered this session, keyed by artwork id (not playlist
  /// id, so changing a playlist's picture invalidates naturally). Each entry
  /// is `COVER_COLS * COVER_ROWS * 6` bytes — under 2 KB — so even the 10k
  /// playlist ceiling caps this at ~17 MB. Left unbounded on purpose.
  pub cover_cache: HashMap<String, CoverArt>,
  /// Set once ffmpeg turns out to be missing or unusable, so we stop trying
  /// on every selection change.
  pub cover_render_disabled: bool,

  pub track_list: Vec<TrackRow>,
  pub track_list_title: String,
  pub track_list_context_uri: Option<String>,
  pub track_list_index: usize,
  /// True while a track-list fetch is in flight. Distinct from `is_loading`,
  /// which tracks the playback poll and therefore flickers every
  /// `poll_interval_ms` regardless of what the main pane is doing.
  pub track_list_loading: bool,
  pub track_list_offset: usize,

  pub search_query: String,
  pub search_results: SearchResults,
  pub search_tab: SearchTab,
  pub has_searched: bool,
  /// True while a search request is in flight.
  pub search_loading: bool,

  pub devices: Vec<Device>,
  pub devices_index: usize,

  pub saved_albums: Vec<SavedAlbum>,
  pub saved_albums_index: usize,
  pub saved_albums_offset: usize,

  pub followed_artists: Vec<FullArtist>,
  pub followed_artists_index: usize,
  pub followed_artists_offset: usize,

  pub artist_view: ArtistView,

  pub saved_shows: Vec<Show>,
  pub saved_shows_index: usize,
  pub saved_shows_offset: usize,

  pub show_episodes: Vec<SimplifiedEpisode>,
  pub show_episodes_title: String,
  pub show_episodes_index: usize,
  pub show_episodes_offset: usize,

  pub queue_current: Option<PlayableItem>,
  pub queue_items: Vec<PlayableItem>,
  pub queue_index: usize,

  pub dialog: Option<Dialog>,

  pub help_visible: bool,
  /// Scroll offset for the help overlay, in lines. Clamped at draw time,
  /// which is the only place the visible height is known.
  pub help_scroll: u16,
}

#[derive(Clone, Debug)]
pub struct Dialog {
  pub message: String,
  pub action: DialogAction,
}

#[derive(Clone, Debug)]
pub enum DialogAction {
  UnfollowPlaylist { playlist_id: String },
}

impl AppState {
  pub fn new(config: Arc<UserConfig>) -> Self {
    let theme = config.theme;
    let warmth = config.behavior.time_of_day_shift.clamp(0.0, 1.0);
    Self {
      config,
      theme,
      last_volume: None,
      volume_changed_at: None,
      time_of_day_shift: warmth,
      theme_mode: ThemeMode::Fixed,
      theme_fixed: theme,
      theme_before_preview: None,
      theme_transition: None,
      theme_picker_index: 0,
      playback: None,
      playback_received_at: None,
      last_error: None,
      is_loading: false,
      active_block: ActiveBlock::Library,
      block_history: Vec::new(),
      library_index: 0,
      playlists: Vec::new(),
      playlists_index: 0,
      playlists_offset: 0,
      playlists_hidden: 0,
      playlist_cover: None,
      now_playing_cover: None,
      cover_cache: HashMap::new(),
      cover_render_disabled: false,
      track_list: Vec::new(),
      track_list_title: String::new(),
      track_list_context_uri: None,
      track_list_index: 0,
      track_list_loading: false,
      track_list_offset: 0,
      search_query: String::new(),
      search_results: SearchResults::default(),
      search_tab: SearchTab::Tracks,
      has_searched: false,
      search_loading: false,
      devices: Vec::new(),
      devices_index: 0,
      saved_albums: Vec::new(),
      saved_albums_index: 0,
      saved_albums_offset: 0,
      followed_artists: Vec::new(),
      followed_artists_index: 0,
      followed_artists_offset: 0,
      artist_view: ArtistView::default(),
      saved_shows: Vec::new(),
      saved_shows_index: 0,
      saved_shows_offset: 0,
      show_episodes: Vec::new(),
      show_episodes_title: String::new(),
      show_episodes_index: 0,
      show_episodes_offset: 0,
      queue_current: None,
      queue_items: Vec::new(),
      queue_index: 0,
      dialog: None,
      help_visible: false,
      help_scroll: 0,
    }
  }

  pub fn device_id(&self) -> Option<&str> {
    self.playback.as_ref().and_then(|p| p.device.id.as_deref())
  }

  pub fn current_volume(&self) -> u8 {
    self
      .playback
      .as_ref()
      .and_then(|p| p.device.volume_percent)
      .map(|v| v.min(100) as u8)
      .unwrap_or(0)
  }

  /// Strength applied when switching the after-dark modifier on. The curve is
  /// already restrained, so full is the sensible on-state.
  pub const AFTER_DARK_ON: f32 = 1.0;

  /// Flip the after-dark modifier, returning the new strength.
  pub fn toggle_after_dark(&mut self) -> f32 {
    self.time_of_day_shift = if self.time_of_day_shift > 0.0 {
      0.0
    } else {
      Self::AFTER_DARK_ON
    };
    self.time_of_day_shift
  }

  pub fn after_dark_on(&self) -> bool {
    self.time_of_day_shift > 0.0
  }

  /// Snapshot the current theme source before previewing.
  pub fn begin_theme_preview(&mut self) {
    self.theme_before_preview = Some(ThemeSnapshot {
      theme: self.theme,
      mode: self.theme_mode,
      fixed: self.theme_fixed,
    });
  }

  /// Restore the snapshot taken by `begin_theme_preview`, if any.
  ///
  /// Snaps rather than fades: animating back would read as the app undoing
  /// itself rather than as a cancellation.
  pub fn cancel_theme_preview(&mut self) {
    if let Some(snap) = self.theme_before_preview.take() {
      self.theme_mode = snap.mode;
      self.theme_fixed = snap.fixed;
      self.set_theme_immediate(snap.theme);
    }
  }

  /// Point the theme at preset `index`, fading over `duration`.
  ///
  /// The single entry point for choosing a theme, so mode and palette can
  /// never disagree. Selecting `DecadeAuto` leaves `theme_fixed` alone on
  /// purpose — it stays the fallback for tracks with no usable release date.
  pub fn select_preset(&mut self, index: usize, duration: Duration) {
    use crate::config::presets::{PresetKind, PRESETS};
    let Some(preset) = PRESETS.get(index) else {
      return;
    };
    match preset.kind {
      PresetKind::Fixed => {
        self.theme_mode = ThemeMode::Fixed;
        if let Some(t) = preset.theme() {
          self.theme_fixed = t;
        }
      }
      PresetKind::DecadeAuto => self.theme_mode = ThemeMode::DecadeAuto,
      PresetKind::EraAuto => self.theme_mode = ThemeMode::EraAuto,
      PresetKind::TimeOfDayAuto => self.theme_mode = ThemeMode::TimeOfDayAuto,
      // Not a source: moving the cursor onto it must not disturb the theme.
      PresetKind::AfterDark => {}
    }
    self.apply_theme_source(duration);
  }

  /// Recompute the target from the active source and fade toward it.
  ///
  /// Called once per frame. `set_theme` is a no-op when the target already
  /// matches, so in `Fixed` mode this costs a comparison and in `DecadeAuto`
  /// it costs a short string parse — and a track change into a different
  /// decade starts a fade on its own without anything having to notify us.
  pub fn apply_theme_source(&mut self, duration: Duration) {
    let base = match self.theme_mode {
      ThemeMode::Fixed => self.theme_fixed,
      // Falls back to the default palette, not to `theme_fixed`: an
      // unknown-year track should look the same whichever theme you happened
      // to be on before switching to decade mode.
      ThemeMode::DecadeAuto => self
        .decade_theme()
        .unwrap_or_else(crate::config::presets::default_theme),
      ThemeMode::EraAuto => self
        .era_theme()
        .unwrap_or_else(crate::config::presets::default_theme),
      ThemeMode::TimeOfDayAuto => crate::config::daylight::theme_now(),
    };
    // Layered on top of the source rather than replacing it, so decade mode
    // still picks the palette and this only warms it after dark.
    let target = self.warmed(base);
    // Never restart a fade that is already heading to this target. This runs
    // every frame, and `set_theme` resets the transition clock, so restarting
    // pins elapsed at ~0 and the fade never advances. With a named colour,
    // which snaps at the midpoint rather than interpolating, t stays below
    // 0.5 forever and the theme never moves at all — the theme appearing to
    // change only after a restart, since startup snaps.
    let already_heading_there = match &self.theme_transition {
      Some(t) => t.to == target,
      None => self.theme == target,
    };
    if already_heading_there {
      return;
    }
    self.set_theme(target, duration);
  }

  /// Apply the configured after-dark warmth, if any.
  fn warmed(&self, theme: Theme) -> Theme {
    let strength = self.time_of_day_shift;
    if strength <= 0.0 {
      return theme;
    }
    crate::config::daylight::warm_theme(theme, strength * crate::config::daylight::warmth_now())
  }

  /// Phase of the day currently in effect, e.g. "dusk".
  pub fn day_phase_label(&self) -> &'static str {
    crate::config::daylight::label_now()
  }

  /// Label of the decade currently in effect, e.g. "1980s". None when the
  /// playing track's year is unknown, or nothing is playing.
  pub fn decade_label(&self) -> Option<&'static str> {
    let year = self.playing_release_year()?;
    Some(crate::config::presets::palette_for_year(year).label)
  }

  /// Smallest cover image URL for whatever is playing.
  ///
  /// Tracks carry images on the album, episodes on the show. `Unknown` items
  /// are read out of the raw JSON for the same reason `playing_uri` is: the
  /// untagged fallback is reached in practice, and skipping it would mean the
  /// playbar cover silently never appearing for those tracks.
  pub fn playing_image_url(&self) -> Option<String> {
    let item = self.playback.as_ref()?.item.as_ref()?;
    match item {
      PlayableItem::Track(t) => smallest_image(&t.album.images),
      PlayableItem::Episode(e) => smallest_image(&e.show.images),
      PlayableItem::Unknown(json) => json
        .get("album")
        .and_then(|a| a.get("images"))
        .and_then(|v| v.as_array())
        .and_then(|arr| {
          arr
            .iter()
            .filter_map(|i| {
              let url = i.get("url")?.as_str()?.to_string();
              let width = i.get("width").and_then(|w| w.as_u64()).unwrap_or(u64::MAX);
              Some((width, url))
            })
            .min_by_key(|(w, _)| *w)
            .map(|(_, url)| url)
        }),
    }
  }

  /// Era-set label for the playing track, e.g. "1950s".
  pub fn era_label(&self) -> Option<&'static str> {
    let year = self.playing_release_year()?;
    Some(crate::config::presets::era_for_year(year).label)
  }

  /// Era-set palette for the playing track's decade, if its year is known.
  pub fn era_theme(&self) -> Option<Theme> {
    let year = self.playing_release_year()?;
    Some(crate::config::presets::era_for_year(year).theme())
  }

  /// Palette for the playing track's decade, if its year can be determined.
  pub fn decade_theme(&self) -> Option<Theme> {
    let year = self.playing_release_year()?;
    Some(crate::config::presets::palette_for_year(year).theme())
  }

  /// Release year of whatever is playing.
  pub fn playing_release_year(&self) -> Option<u16> {
    let item = self.playback.as_ref()?.item.as_ref()?;
    let raw = match item {
      PlayableItem::Track(t) => t.album.release_date.clone(),
      PlayableItem::Episode(e) => Some(e.release_date.clone()),
      // Same lesson as `playing_uri`: the untagged fallback is reached in
      // practice, and giving up here would mean decade mode silently never
      // engaging for exactly those tracks.
      PlayableItem::Unknown(json) => json
        .get("album")
        .and_then(|a| a.get("release_date"))
        .and_then(|v| v.as_str())
        .map(str::to_string),
    };
    parse_release_year(raw.as_deref()?)
  }

  /// Begin fading to `target`. Called by every theme source.
  ///
  /// A change mid-fade starts the next one from whatever is on screen right
  /// now, not from the previous target, so rapid changes chase smoothly
  /// instead of jumping back.
  pub fn set_theme(&mut self, target: Theme, duration: Duration) {
    if duration.is_zero() || self.theme == target {
      self.theme = target;
      self.theme_transition = None;
      return;
    }
    self.theme_transition = Some(ThemeTransition {
      from: self.theme,
      to: target,
      started: Instant::now(),
      duration,
    });
  }

  /// Apply a theme with no fade — startup, and cancelling a preview, where
  /// animating from an unrelated palette would look like a glitch.
  pub fn set_theme_immediate(&mut self, target: Theme) {
    self.theme = target;
    self.theme_transition = None;
  }

  /// Advance the fade. Called once per draw, so the rendered theme is always
  /// current for the frame about to be painted.
  pub fn tick_theme(&mut self) {
    let Some(t) = self.theme_transition else {
      return;
    };
    let elapsed = t.started.elapsed();
    if elapsed >= t.duration {
      self.theme = t.to;
      self.theme_transition = None;
      return;
    }
    let linear = elapsed.as_secs_f32() / t.duration.as_secs_f32();
    // Ease-out: fast to start, settling at the end. A linear fade reads as a
    // mechanical wipe; this reads as the UI arriving somewhere.
    let eased = 1.0 - (1.0 - linear) * (1.0 - linear);
    self.theme = t.from.blend(t.to, eased);
  }

  /// How long the volume figure stays lit after a change.
  pub const VOLUME_FLASH: Duration = Duration::from_millis(700);

  /// Note the current volume, starting a flash if it moved.
  ///
  /// Called once per draw. The first observation only records the baseline —
  /// otherwise the figure would flash on startup, when nothing changed.
  pub fn tick_volume(&mut self) {
    let Some(now) = self.playback.as_ref().and_then(|p| p.device.volume_percent) else {
      return;
    };
    match self.last_volume {
      Some(prev) if prev != now => self.volume_changed_at = Some(Instant::now()),
      Some(_) => {}
      None => {}
    }
    self.last_volume = Some(now);
  }

  /// How lit the volume figure should be, 1.0 just after a change down to 0.0.
  pub fn volume_flash(&self) -> f32 {
    let Some(at) = self.volume_changed_at else {
      return 0.0;
    };
    let elapsed = at.elapsed();
    if elapsed >= Self::VOLUME_FLASH {
      return 0.0;
    }
    1.0 - elapsed.as_secs_f32() / Self::VOLUME_FLASH.as_secs_f32()
  }

  /// True while anything on screen is mid-animation, so the main loop can
  /// redraw faster than the configured tick. Volume is included because a
  /// three-frame fade reads as a stutter, the same reason theme fades are.
  pub fn needs_fast_redraw(&self) -> bool {
    self.theme_transition_active() || self.volume_flash() > 0.0
  }

  /// Whether a fade is running. The main loop redraws faster while one is, so
  /// it does not render as three discrete steps.
  pub fn theme_transition_active(&self) -> bool {
    self.theme_transition.is_some()
  }

  pub fn is_playing(&self) -> bool {
    self
      .playback
      .as_ref()
      .map(|p| p.is_playing)
      .unwrap_or(false)
  }

  pub fn current_progress_ms(&self) -> Option<i64> {
    self
      .playback
      .as_ref()
      .and_then(|p| p.progress.map(|d| d.num_milliseconds()))
  }

  /// Progress extrapolated since the last poll, so the bar moves between polls.
  pub fn extrapolated_progress_ms(&self) -> Option<i64> {
    let base = self.current_progress_ms()?;
    let is_playing = self.is_playing();
    if !is_playing {
      return Some(base);
    }
    let elapsed = self
      .playback_received_at
      .map(|t| t.elapsed().as_millis() as i64)
      .unwrap_or(0);
    Some(base + elapsed)
  }

  /// URI of whatever is playing — track or episode.
  ///
  /// Returns None for anything without a URI (local files, unrecognised
  /// items). Callers comparing against `TrackRow::uri`, which is also
  /// optional, must check this is `Some` first: two `None`s compare equal and
  /// would mark every unplayable row as playing.
  pub fn playing_uri(&self) -> Option<String> {
    use rspotify::prelude::Id;
    self.playback.as_ref().and_then(|p| match p.item.as_ref()? {
      PlayableItem::Track(t) => t.id.as_ref().map(|id| id.uri()),
      PlayableItem::Episode(e) => Some(e.id.uri()),
      // `PlayableItem` is #[serde(untagged)] with an Unknown(Value) fallback,
      // so an item whose shape drifts from rspotify's FullTrack lands here
      // silently rather than failing. That is not hypothetical: the playbar
      // (`ui::playbar::unknown_from_json`) and the playlist item mapper both
      // already handle it, because Spotify's responses do reach it.
      //
      // Returning None here meant the playbar could show a track while the
      // now-playing marker had nothing to match against — visible as a
      // playing song with no marker on its row. Spotify always sends `uri`,
      // so read it straight out of the raw JSON.
      PlayableItem::Unknown(json) => json.get("uri").and_then(|v| v.as_str()).map(str::to_string),
    })
  }

  pub fn current_track_id(&self) -> Option<String> {
    use rspotify::prelude::Id;
    self.playback.as_ref().and_then(|p| match p.item.as_ref()? {
      PlayableItem::Track(t) => t.id.as_ref().map(|id| id.id().to_string()),
      _ => None,
    })
  }

  pub fn current_album_id(&self) -> Option<String> {
    use rspotify::prelude::Id;
    self.playback.as_ref().and_then(|p| match p.item.as_ref()? {
      PlayableItem::Track(t) => t.album.id.as_ref().map(|id| id.id().to_string()),
      _ => None,
    })
  }

  pub fn current_artist_id(&self) -> Option<String> {
    use rspotify::prelude::Id;
    self.playback.as_ref().and_then(|p| match p.item.as_ref()? {
      PlayableItem::Track(t) => t
        .artists
        .iter()
        .find_map(|a| a.id.as_ref().map(|id| id.id().to_string())),
      _ => None,
    })
  }

  pub fn push_block(&mut self, new: ActiveBlock) {
    self.block_history.push(self.active_block);
    self.active_block = new;
  }

  pub fn pop_block(&mut self) -> bool {
    if let Some(prev) = self.block_history.pop() {
      self.active_block = prev;
      true
    } else {
      false
    }
  }
}

#[cfg(test)]
mod theme_transition_tests {
  use super::*;

  /// Shared with `draw_loop_tests`: a theme clearly unlike any bundled
  /// palette, so inheriting one instead of the intended value shows up.
  pub(super) fn rgb_theme(v: u8) -> Theme {
    let base = Theme::from(&crate::config::theme::ThemeCfg::default());
    Theme {
      active: ratatui::style::Color::Rgb(v, v, v),
      progress: ratatui::style::Color::Rgb(v, v, v),
      ..base
    }
  }
  use crate::config::user::UserConfig;
  use ratatui::style::Color;

  fn state() -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    AppState::new(Arc::new(cfg))
  }

  #[test]
  fn release_year_accepts_every_spotify_precision() {
    // release_date_precision is year, month or day.
    assert_eq!(parse_release_year("1984"), Some(1984));
    assert_eq!(parse_release_year("1984-05"), Some(1984));
    assert_eq!(parse_release_year("1984-05-12"), Some(1984));
  }

  #[test]
  fn release_year_rejects_junk_without_panicking() {
    for bad in ["", "abc", "20", "20x4", "not-a-date", "0001", "9999"] {
      assert_eq!(parse_release_year(bad), None, "{bad:?} must be rejected");
    }
    // Multi-byte input: str::get must refuse the split, not panic.
    assert_eq!(parse_release_year("😀😀😀😀"), None);
    assert_eq!(parse_release_year("19😀4"), None);
  }

  /// With no year to go on, decade mode shows the default palette rather than
  /// whatever was selected before — so the same unknown-year track does not
  /// look different depending on where you came from.
  #[test]
  fn decade_mode_falls_back_to_the_default_palette() {
    let mut s = state();
    // Somewhere clearly unrelated to the default, so inheriting would show.
    s.theme_fixed = rgb_theme(77);
    s.set_theme_immediate(s.theme_fixed);
    s.theme_mode = ThemeMode::DecadeAuto;

    assert!(s.decade_theme().is_none(), "no playback, no decade");
    s.apply_theme_source(Duration::ZERO);
    assert_eq!(
      s.theme,
      crate::config::presets::default_theme(),
      "falls back to Spotify Green, not to the previous palette"
    );
  }

  /// The lesson from the now-playing marker: `PlayableItem` is untagged with
  /// an Unknown fallback that is reached in practice. Decade mode has to read
  /// the raw JSON too, or it silently never engages for those tracks.
  #[test]
  fn decade_resolves_from_an_unparsed_playback_item() {
    let mut s = state();
    let raw = serde_json::json!({
      "device": {
        "id": "d", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "T", "type": "Computer",
        "volume_percent": 10
      },
      "repeat_state": "off", "shuffle_state": false, "context": null,
      "timestamp": 1_767_225_600_000i64, "progress_ms": 0, "is_playing": true,
      "currently_playing_type": "track", "actions": { "disallows": {} },
      "item": {
        "name": "X", "uri": "spotify:track:x",
        "album": { "release_date": "1987-03-01" }
      }
    });
    let playback: rspotify::model::CurrentPlaybackContext =
      serde_json::from_value(raw).expect("must parse via the Unknown fallback");
    assert!(
      playback.item.as_ref().unwrap().is_unknown(),
      "must be Unknown"
    );
    s.playback = Some(playback);

    assert_eq!(s.playing_release_year(), Some(1987));
    assert_eq!(s.decade_label(), Some("1980s"));
    assert!(s.decade_theme().is_some());
  }

  /// Selecting a fixed palette after auto must leave auto behind entirely.
  #[test]
  fn selecting_a_fixed_preset_leaves_decade_mode() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    let auto = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::DecadeAuto)
      .expect("an auto preset must exist");
    s.select_preset(auto, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::DecadeAuto);

    s.select_preset(0, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::Fixed);
    assert_eq!(
      s.theme_fixed.active,
      PRESETS[0].theme().unwrap().active,
      "fixed palette applied"
    );
  }

  /// Choosing auto must not overwrite `theme_fixed`: it is what you return to
  /// when you leave decade mode, so losing it would strand you on a palette
  /// you never picked.
  #[test]
  fn choosing_auto_preserves_the_palette_to_return_to() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    s.select_preset(1, Duration::ZERO);
    let fallback = s.theme_fixed;

    let auto = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::DecadeAuto)
      .unwrap();
    s.select_preset(auto, Duration::ZERO);
    assert_eq!(s.theme_fixed, fallback, "fallback untouched");
  }

  #[test]
  fn out_of_range_preset_index_is_ignored() {
    let mut s = state();
    let before = s.theme_mode;
    s.select_preset(9999, Duration::ZERO);
    assert_eq!(s.theme_mode, before);
  }

  #[test]
  fn zero_duration_snaps_and_starts_no_transition() {
    let mut s = state();
    let target = rgb_theme(200);
    s.set_theme(target, Duration::ZERO);
    assert_eq!(s.theme.active, target.active);
    assert!(!s.theme_transition_active(), "nothing left running");
  }

  /// Setting the theme it already has must not start a fade — otherwise the
  /// main loop would spin at 30fps for no visible reason.
  #[test]
  fn setting_the_current_theme_is_a_no_op() {
    let mut s = state();
    let same = s.theme;
    s.set_theme(same, Duration::from_millis(350));
    assert!(!s.theme_transition_active());
  }

  #[test]
  fn transition_completes_and_lands_exactly_on_the_target() {
    let mut s = state();
    s.set_theme_immediate(rgb_theme(0));
    let target = rgb_theme(255);
    s.set_theme(target, Duration::from_millis(20));
    assert!(s.theme_transition_active(), "fade started");

    std::thread::sleep(Duration::from_millis(40));
    s.tick_theme();

    assert_eq!(s.theme.active, target.active, "ends on the target exactly");
    assert!(!s.theme_transition_active(), "and clears itself");
  }

  /// A fade interrupted mid-flight must continue from what is on screen, not
  /// from the previous start colour — otherwise rapid picker presses jump
  /// backwards before moving on.
  #[test]
  fn interrupting_a_fade_starts_from_the_rendered_colour() {
    let mut s = state();
    s.set_theme_immediate(rgb_theme(0));
    s.set_theme(rgb_theme(255), Duration::from_millis(200));

    std::thread::sleep(Duration::from_millis(60));
    s.tick_theme();
    let midway = s.theme.active;
    assert_ne!(midway, Color::Rgb(0, 0, 0), "actually moved");
    assert_ne!(midway, Color::Rgb(255, 255, 255), "but not finished");

    // Redirect to a third theme.
    s.set_theme(rgb_theme(64), Duration::from_millis(200));
    s.tick_theme();
    // The new fade's start point is where the screen already was.
    let Some(t) = s.theme_transition else {
      panic!("expected an active transition");
    };
    assert_eq!(t.from, midway_theme(midway, &s), "resumes from the screen");
  }

  fn midway_theme(active: Color, s: &AppState) -> Theme {
    Theme { active, ..s.theme }
  }

  /// tick_theme with nothing running must not touch the theme.
  #[test]
  fn tick_without_a_transition_is_inert() {
    let mut s = state();
    let before = s.theme;
    s.tick_theme();
    assert_eq!(s.theme, before);
  }
}

#[cfg(test)]
mod draw_loop_tests {
  use super::theme_transition_tests::rgb_theme;
  use super::*;
  use crate::config::user::UserConfig;

  fn state() -> AppState {
    let cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    AppState::new(Arc::new(cfg))
  }

  fn state_with_warmth(strength: f32) -> AppState {
    let mut cfg = UserConfig::load_or_default(std::path::Path::new(
      "/nonexistent/spotuify-test-config.yml",
    ))
    .unwrap();
    cfg.behavior.time_of_day_shift = strength;
    AppState::new(Arc::new(cfg))
  }

  /// The time-of-day modifier is recomputed every frame from the wall clock,
  /// which is the same shape as the bug that froze fades: a target that keeps
  /// changing restarts the transition forever and it never lands.
  ///
  /// Cannot assert the theme is *warmed* — that depends on the clock when the
  /// test runs, and at midday warmth is legitimately zero. The property that
  /// must hold at any hour is that it settles.
  #[test]
  fn warmth_modifier_still_settles() {
    let mut s = state_with_warmth(1.0);
    let fade = Duration::from_millis(120);
    s.select_preset(1, fade);

    for frame in 0..300 {
      s.apply_theme_source(fade);
      s.tick_theme();
      if !s.theme_transition_active() {
        // Settled. Confirm it stays settled rather than immediately
        // re-triggering on the next frame.
        for _ in 0..20 {
          s.apply_theme_source(fade);
          s.tick_theme();
        }
        assert!(
          !s.theme_transition_active(),
          "settled at frame {frame} then restarted itself"
        );
        return;
      }
      std::thread::sleep(Duration::from_millis(3));
    }
    panic!("never settled with the warmth modifier active");
  }

  /// The first observation must not flash: nothing changed, we just started
  /// looking. Otherwise every launch opens with the volume lit.
  #[test]
  fn first_volume_observation_does_not_flash() {
    let mut s = state();
    s.playback = Some(playing_with_volume(50));
    s.tick_volume();
    assert_eq!(s.volume_flash(), 0.0, "baseline only");
    assert_eq!(s.last_volume, Some(50));
  }

  #[test]
  fn a_volume_change_flashes_and_decays() {
    let mut s = state();
    s.playback = Some(playing_with_volume(50));
    s.tick_volume();

    s.playback = Some(playing_with_volume(60));
    s.tick_volume();
    let lit = s.volume_flash();
    assert!(lit > 0.9, "flashes bright immediately, got {lit}");
    assert!(s.needs_fast_redraw(), "and asks for a faster redraw");

    std::thread::sleep(Duration::from_millis(60));
    let later = s.volume_flash();
    assert!(later < lit, "decays: {later} should be under {lit}");
    assert!(later > 0.0, "but is still lit");
  }

  #[test]
  fn an_unchanged_volume_does_not_reflash() {
    let mut s = state();
    s.playback = Some(playing_with_volume(50));
    s.tick_volume();
    s.tick_volume();
    s.tick_volume();
    assert_eq!(s.volume_flash(), 0.0, "polling must not retrigger");
  }

  #[test]
  fn the_flash_expires() {
    let mut s = state();
    s.playback = Some(playing_with_volume(50));
    s.tick_volume();
    s.playback = Some(playing_with_volume(70));
    s.tick_volume();

    s.volume_changed_at = Some(Instant::now() - AppState::VOLUME_FLASH);
    assert_eq!(s.volume_flash(), 0.0, "expired");
    assert!(
      !s.needs_fast_redraw(),
      "and stops asking for a faster redraw, or the loop spins at 30fps"
    );
  }

  /// Missing volume is a normal state — some devices don't report it.
  #[test]
  fn absent_volume_is_handled() {
    let mut s = state();
    s.tick_volume();
    assert_eq!(s.volume_flash(), 0.0);
    assert_eq!(s.last_volume, None);
  }

  fn playing_with_volume(vol: u32) -> rspotify::model::CurrentPlaybackContext {
    let raw = serde_json::json!({
      "device": {
        "id": "d", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "T", "type": "Computer",
        "volume_percent": vol
      },
      "repeat_state": "off", "shuffle_state": false, "context": null,
      "timestamp": 1_767_225_600_000i64, "progress_ms": 0, "is_playing": true,
      "currently_playing_type": "track", "actions": { "disallows": {} },
      "item": { "name": "X", "uri": "spotify:track:x", "album": {} }
    });
    serde_json::from_value(raw).expect("fixture must parse")
  }

  /// Era mode is a second decade source, so it needs the same guarantees:
  /// it settles, and it falls back to the default rather than inheriting.
  #[test]
  fn era_mode_settles_and_falls_back_to_the_default() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    let fade = Duration::from_millis(120);
    let row = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::EraAuto)
      .expect("the era entry must exist");

    s.theme_fixed = rgb_theme(77);
    s.select_preset(row, fade);
    assert_eq!(s.theme_mode, ThemeMode::EraAuto);

    for _ in 0..300 {
      s.apply_theme_source(fade);
      s.tick_theme();
      if !s.theme_transition_active() {
        break;
      }
      std::thread::sleep(Duration::from_millis(3));
    }
    assert!(!s.theme_transition_active(), "era mode must settle");
    assert!(s.era_theme().is_none(), "nothing playing, no era");
    assert_eq!(
      s.theme,
      crate::config::presets::default_theme(),
      "falls back to Spotify Green, not the previous palette"
    );
  }

  /// The two decade sets must be independently selectable, and switching
  /// between them must actually change mode rather than sticking.
  #[test]
  fn decade_and_era_modes_are_distinct() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    let decade = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::DecadeAuto)
      .unwrap();
    let era = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::EraAuto)
      .unwrap();
    assert_ne!(decade, era, "separate entries");

    s.select_preset(decade, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::DecadeAuto);
    s.select_preset(era, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::EraAuto);
    s.select_preset(decade, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::DecadeAuto);
  }

  /// The same track must resolve differently under the two sets, or one of
  /// them is silently pointing at the wrong table.
  #[test]
  fn the_two_sets_give_different_palettes_for_the_same_year() {
    let mut s = state();
    s.playback = Some(playing_from_year("1965"));
    assert_eq!(s.decade_label(), Some("1960s"));
    assert_eq!(s.era_label(), Some("1960s"));
    assert_ne!(
      s.decade_theme(),
      s.era_theme(),
      "1960s differs between the sets — mustard vs psychedelic orange"
    );
  }

  fn playing_from_year(date: &str) -> rspotify::model::CurrentPlaybackContext {
    let raw = serde_json::json!({
      "device": {
        "id": "d", "is_active": true, "is_private_session": false,
        "is_restricted": false, "name": "T", "type": "Computer",
        "volume_percent": 10
      },
      "repeat_state": "off", "shuffle_state": false, "context": null,
      "timestamp": 1_767_225_600_000i64, "progress_ms": 0, "is_playing": true,
      "currently_playing_type": "track", "actions": { "disallows": {} },
      "item": {
        "name": "X", "uri": "spotify:track:x",
        "album": { "release_date": date }
      }
    });
    serde_json::from_value(raw).expect("fixture must parse")
  }

  /// Same failure mode as the warmth modifier: the day cycle target drifts
  /// with the clock, so it must still settle rather than restarting forever.
  #[test]
  fn day_cycle_mode_settles() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    let fade = Duration::from_millis(120);
    let row = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::TimeOfDayAuto)
      .expect("the day-cycle entry must exist");

    s.select_preset(row, fade);
    assert_eq!(s.theme_mode, ThemeMode::TimeOfDayAuto);

    for frame in 0..300 {
      s.apply_theme_source(fade);
      s.tick_theme();
      if !s.theme_transition_active() {
        for _ in 0..20 {
          s.apply_theme_source(fade);
          s.tick_theme();
        }
        assert!(
          !s.theme_transition_active(),
          "settled at frame {frame} then restarted itself"
        );
        return;
      }
      std::thread::sleep(Duration::from_millis(3));
    }
    panic!("day cycle never settled");
  }

  /// Switching away from the day cycle must restore the chosen palette, not
  /// leave the clock in charge.
  #[test]
  fn leaving_day_cycle_returns_to_the_fixed_palette() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    s.select_preset(1, Duration::ZERO);
    let chosen = s.theme_fixed;

    let row = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::TimeOfDayAuto)
      .unwrap();
    s.select_preset(row, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::TimeOfDayAuto);

    s.select_preset(1, Duration::ZERO);
    assert_eq!(s.theme_mode, ThemeMode::Fixed);
    assert_eq!(s.theme, chosen, "back to the chosen palette");
  }

  /// The toggle must not behave like a theme: moving onto it or selecting it
  /// must leave the chosen palette alone, or picking a theme then switching
  /// the modifier would silently lose the theme.
  #[test]
  fn the_after_dark_row_is_not_a_theme_source() {
    use crate::config::presets::{PresetKind, PRESETS};
    let mut s = state();
    s.select_preset(1, Duration::ZERO);
    let chosen = s.theme_fixed;
    let mode = s.theme_mode;

    let row = PRESETS
      .iter()
      .position(|p| p.kind == PresetKind::AfterDark)
      .expect("the toggle row must exist");
    s.select_preset(row, Duration::ZERO);

    assert_eq!(s.theme_fixed, chosen, "palette preserved");
    assert_eq!(s.theme_mode, mode, "mode preserved");
  }

  #[test]
  fn toggling_after_dark_flips_and_reports_the_new_state() {
    let mut s = state_with_warmth(0.0);
    assert!(!s.after_dark_on());

    let on = s.toggle_after_dark();
    assert!(on > 0.0 && s.after_dark_on(), "switched on");

    let off = s.toggle_after_dark();
    assert_eq!(off, 0.0);
    assert!(!s.after_dark_on(), "and back off");
  }

  /// Switching the modifier must not disturb which theme is selected.
  #[test]
  fn toggling_after_dark_keeps_the_selected_source() {
    let mut s = state_with_warmth(0.0);
    s.select_preset(2, Duration::ZERO);
    let chosen = s.theme_fixed;

    s.toggle_after_dark();
    s.apply_theme_source(Duration::ZERO);
    assert_eq!(s.theme_fixed, chosen, "source untouched by the modifier");
  }

  /// Zero strength must be exactly the unmodified source, at any hour.
  #[test]
  fn zero_warmth_matches_the_unmodified_source() {
    use crate::config::presets::PRESETS;
    let mut s = state_with_warmth(0.0);
    s.select_preset(1, Duration::ZERO);
    assert_eq!(s.theme, PRESETS[1].theme().unwrap());
  }

  /// Cancelling a preview must survive the next frame. `apply_theme_source`
  /// recomputes from `theme_mode`/`theme_fixed`, so restoring only `theme`
  /// leaves the previewed palette as the source and the next frame fades
  /// straight back to it.
  #[test]
  fn cancelling_a_preview_is_not_undone_by_the_next_frame() {
    use crate::config::presets::PRESETS;
    let mut s = state();
    let original = s.theme;

    // Open the picker and preview a different preset.
    s.begin_theme_preview();
    s.select_preset(1, Duration::ZERO);
    assert_ne!(s.theme, original, "preview changed the theme");

    // Cancel.
    s.cancel_theme_preview();
    assert_eq!(s.theme, original, "cancel restored the theme");

    // Now let the draw loop run.
    for _ in 0..20 {
      s.apply_theme_source(Duration::ZERO);
      s.tick_theme();
    }
    assert_eq!(
      s.theme,
      original,
      "cancel must hold; got {:?} which is preset 1's {:?}",
      s.theme.active,
      PRESETS[1].theme().unwrap().active
    );
  }

  /// Simulates what `ui::draw` does every frame: recompute the source, then
  /// advance the fade. A theme picked in the picker must actually arrive.
  #[test]
  fn a_selected_theme_converges_within_a_reasonable_number_of_frames() {
    use crate::config::presets::PRESETS;
    let mut s = state();
    let fade = Duration::from_millis(350);

    s.select_preset(1, fade);
    let target = PRESETS[1].theme().unwrap();

    for frame in 0..200 {
      s.apply_theme_source(fade);
      s.tick_theme();
      if s.theme == target && !s.theme_transition_active() {
        println!("converged after {frame} frames");
        return;
      }
      std::thread::sleep(Duration::from_millis(5));
    }
    panic!(
      "theme never landed on the target.\n  want {:?}\n  got  {:?}\n  transition still active: {}",
      target.active,
      s.theme.active,
      s.theme_transition_active()
    );
  }
}
