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
use std::sync::Arc;
use std::time::Instant;

pub const LIBRARY_ENTRIES: [&str; 5] = [
  "Liked Songs",
  "Albums",
  "Artists",
  "Podcasts",
  "Recently Played",
];

/// Cover art is drawn with the upper-half-block glyph: each terminal cell
/// carries two independently coloured pixels (foreground = top, background =
/// bottom), so a cell grid of `COVER_COLS x COVER_ROWS` renders
/// `COVER_COLS x (COVER_ROWS * 2)` full-colour pixels.
///
/// 24x12 cells fits the 36 usable columns of the sidebar and costs 12 rows.
/// Terminal cells are roughly 2:1, so this is square on screen.
pub const COVER_COLS: u16 = 24;
pub const COVER_ROWS: u16 = 12;

/// One rendered cover: `COVER_ROWS * COVER_COLS` cells, row-major, each a
/// (top, bottom) RGB pair.
#[derive(Clone, Debug)]
pub struct CoverArt {
  pub cols: u16,
  pub rows: u16,
  pub cells: Vec<(Rgb, Rgb)>,
}

pub type Rgb = (u8, u8, u8);

/// What we know about the selected playlist's cover. `art: None` means we
/// looked and Spotify had no image — distinct from `playlist_cover: None`,
/// which means we have not looked yet. Keeping them apart stops the network
/// task from re-spawning ffmpeg for a playlist that will never have art.
#[derive(Clone, Debug)]
pub struct PlaylistCover {
  pub playlist_id: String,
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
  /// Saved theme so the picker can revert on cancel.
  pub theme_before_preview: Option<Theme>,
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
  /// Rendered cover for the currently selected playlist, if any.
  pub playlist_cover: Option<PlaylistCover>,
  /// Set once ffmpeg turns out to be missing or unusable, so we stop trying
  /// on every selection change.
  pub cover_render_disabled: bool,

  pub track_list: Vec<TrackRow>,
  pub track_list_title: String,
  pub track_list_context_uri: Option<String>,
  pub track_list_index: usize,
  pub track_list_offset: usize,

  pub search_query: String,
  pub search_results: SearchResults,
  pub search_tab: SearchTab,
  pub has_searched: bool,

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
    Self {
      config,
      theme,
      theme_before_preview: None,
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
      playlist_cover: None,
      cover_render_disabled: false,
      track_list: Vec::new(),
      track_list_title: String::new(),
      track_list_context_uri: None,
      track_list_index: 0,
      track_list_offset: 0,
      search_query: String::new(),
      search_results: SearchResults::default(),
      search_tab: SearchTab::Tracks,
      has_searched: false,
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
