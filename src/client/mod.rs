use crate::app::{AppState, CoverArt, PlaylistCover, Rgb, TrackRow, COVER_COLS, COVER_ROWS};
use anyhow::{Context, Result};
use rspotify::model::playlist::SimplifiedPlaylist;
use rspotify::model::{
  AdditionalType, AlbumId, AlbumType, ArtistId, EpisodeId, LibraryId, Market, Offset,
  PlayContextId, PlayableId, PlayableItem, PlaylistId, SearchResult, SearchType, ShowId,
  SimplifiedAlbum, TrackId,
};
use rspotify::{prelude::*, AuthCodeSpotify};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;

#[derive(Debug, Clone)]
pub enum IoEvent {
  GetCurrentPlayback,
  GetPlaylists,
  /// Render cover art for whichever playlist is selected *now*. Carries no
  /// id on purpose: the network task re-reads the selection when it dequeues,
  /// so a burst of these from held-down j/k collapses to a single render.
  RefreshPlaylistCover,
  GetPlaylistTracks {
    playlist_id: String,
    playlist_name: String,
  },
  GetSavedTracks,
  GetAlbumTracks {
    album_id: String,
    album_name: String,
  },
  OpenArtist {
    artist_id: String,
    artist_name: String,
  },
  Search(String),
  GetDevices,
  TransferPlayback(String),
  GetSavedAlbums,
  GetFollowedArtists,
  GetRecentlyPlayed,
  ToggleSaveTrack(String),
  ToggleSaveAlbum(String),
  ToggleFollowArtist(String),
  UnfollowPlaylist(String),
  GetSavedShows,
  GetShowEpisodes {
    show_id: String,
    show_name: String,
  },
  GetQueue,
  AddToQueue(String),
  PlayUri(String),
  PausePlayback,
  ResumePlayback,
  NextTrack,
  PreviousTrack,
  ChangeVolume(u8),
  Seek(i64),
  PlayTrackInContext {
    context_uri: String,
    offset_index: usize,
    /// URI of the selected track, when known. Preferred over `offset_index`
    /// because Spotify's positional offset can desync from the local list when
    /// the playlist contains episodes, local files, or market-restricted
    /// items — Spotify still counts those slots but indexes by playable
    /// position. URI offset sidesteps that.
    track_uri: Option<String>,
  },
  PlayTrackUris {
    uris: Vec<String>,
    offset_index: usize,
  },
  Shutdown,
}

pub struct Network {
  spotify: AuthCodeSpotify,
  state: Arc<Mutex<AppState>>,
}

impl Network {
  pub fn new(spotify: AuthCodeSpotify, state: Arc<Mutex<AppState>>) -> Self {
    Self { spotify, state }
  }

  pub async fn run(self, mut rx: mpsc::Receiver<IoEvent>) {
    while let Some(event) = rx.recv().await {
      let name = format!("{event:?}");
      if let Err(err) = self.dispatch(event).await {
        warn!(%name, ?err, "network event failed");
        self.set_error(format!("{name}: {err:#}"));
      }
    }
  }

  async fn dispatch(&self, event: IoEvent) -> Result<()> {
    match event {
      IoEvent::Shutdown => Ok(()),
      IoEvent::GetCurrentPlayback => self.get_current_playback().await,
      IoEvent::GetPlaylists => self.get_playlists().await,
      IoEvent::RefreshPlaylistCover => self.refresh_playlist_cover().await,
      IoEvent::GetPlaylistTracks {
        playlist_id,
        playlist_name,
      } => self.get_playlist_tracks(&playlist_id, &playlist_name).await,
      IoEvent::GetSavedTracks => self.get_saved_tracks().await,
      IoEvent::GetAlbumTracks {
        album_id,
        album_name,
      } => self.get_album_tracks(&album_id, &album_name).await,
      IoEvent::OpenArtist {
        artist_id,
        artist_name,
      } => self.open_artist(&artist_id, &artist_name).await,
      IoEvent::Search(q) => self.search(&q).await,
      IoEvent::GetDevices => self.get_devices().await,
      IoEvent::TransferPlayback(device_id) => self.transfer_playback(&device_id).await,
      IoEvent::GetSavedAlbums => self.get_saved_albums().await,
      IoEvent::GetFollowedArtists => self.get_followed_artists().await,
      IoEvent::GetRecentlyPlayed => self.get_recently_played().await,
      IoEvent::ToggleSaveTrack(track_id) => self.toggle_save_track(&track_id).await,
      IoEvent::ToggleSaveAlbum(album_id) => self.toggle_save_album(&album_id).await,
      IoEvent::ToggleFollowArtist(artist_id) => self.toggle_follow_artist(&artist_id).await,
      IoEvent::UnfollowPlaylist(playlist_id) => self.unfollow_playlist(&playlist_id).await,
      IoEvent::GetSavedShows => self.get_saved_shows().await,
      IoEvent::GetShowEpisodes { show_id, show_name } => {
        self.get_show_episodes(&show_id, &show_name).await
      }
      IoEvent::GetQueue => self.get_queue().await,
      IoEvent::AddToQueue(uri) => self.add_to_queue(&uri).await,
      IoEvent::PlayUri(uri) => self.play_single_uri(&uri).await,
      IoEvent::PausePlayback => self.pause_playback().await,
      IoEvent::ResumePlayback => self.resume_playback().await,
      IoEvent::NextTrack => self.next_track().await,
      IoEvent::PreviousTrack => self.previous_track().await,
      IoEvent::ChangeVolume(v) => self.change_volume(v).await,
      IoEvent::Seek(ms) => self.seek(ms).await,
      IoEvent::PlayTrackInContext {
        context_uri,
        offset_index,
        track_uri,
      } => {
        self
          .play_context(&context_uri, offset_index, track_uri.as_deref())
          .await
      }
      IoEvent::PlayTrackUris { uris, offset_index } => self.play_uris(uris, offset_index).await,
    }
  }

  async fn get_current_playback(&self) -> Result<()> {
    self.set_loading(true);
    let additional_types = [AdditionalType::Track, AdditionalType::Episode];
    let playback = self
      .spotify
      .current_playback(None, Some(&additional_types))
      .await?;
    let mut state = self.state.lock().unwrap();
    state.playback = playback;
    state.playback_received_at = Some(std::time::Instant::now());
    state.is_loading = false;
    state.last_error = None;
    Ok(())
  }

  async fn get_playlists(&self) -> Result<()> {
    // `/me/playlists` caps `limit` at 50, so a single request silently truncates
    // any library with more than 50 playlists. Page until Spotify says there is
    // no next page.
    //
    // The end signal is `page.next == None`, not a short page: Spotify filters
    // unavailable entries out of a page *after* applying `limit`, so a page of
    // 47 does not mean the list is exhausted. Bailing on a short page is what
    // makes playlists go missing from the middle of the list.
    //
    // 10k matches Spotify's per-user playlist hard limit and keeps us well
    // inside the documented 100k max offset.
    const PAGE_LIMIT: u32 = 50;
    const MAX_PLAYLISTS: usize = 10_000;

    // Resolved before paging so a failure here just disables filtering rather
    // than failing the whole load.
    let only_own = {
      let s = self.state.lock().unwrap();
      s.config.behavior.only_own_playlists
    };
    let me: Option<String> = if only_own {
      match self.spotify.current_user().await {
        Ok(user) => Some(user.id.id().to_string()),
        Err(err) => {
          warn!(
            ?err,
            "could not resolve current user; showing all playlists"
          );
          None
        }
      }
    } else {
      None
    };

    let mut playlists = Vec::new();
    let mut offset: u32 = 0;
    loop {
      let page = self
        .spotify
        .current_user_playlists_manual(Some(PAGE_LIMIT), Some(offset))
        .await?;
      let has_next = page.next.is_some();
      playlists.extend(page.items);
      if !has_next || playlists.len() >= MAX_PLAYLISTS {
        break;
      }
      offset += PAGE_LIMIT;
    }

    // Spotify's Feb 2026 restriction means `/playlists/{id}/items` 403s for
    // playlists we don't own, so listing them produces an error row rather
    // than tracks. Hide them by default, but count what we dropped — a list
    // that is silently short is the bug we just fixed in pagination.
    let mut hidden = 0;
    if let Some(me) = &me {
      let before = playlists.len();
      playlists.retain(|p: &SimplifiedPlaylist| p.owner.id.id() == me.as_str());
      hidden = before - playlists.len();
    }

    // Scoped so the guard is structurally dropped before the await below —
    // an explicit drop() still leaves it in the future's captured state and
    // makes the whole network task non-Send.
    {
      let mut state = self.state.lock().unwrap();
      state.playlists = playlists;
      state.playlists_hidden = hidden;
      // Clamp rather than reset — this runs again after an unfollow, and the
      // user's cursor should stay where it was.
      state.playlists_index = state
        .playlists_index
        .min(state.playlists.len().saturating_sub(1));
      // A new list invalidates whatever cover we were showing.
      state.playlist_cover = None;
    }
    // Render art for the initially selected playlist so the pane is populated
    // before the user touches the cursor.
    self.refresh_playlist_cover().await
  }

  /// Render the selected playlist's cover into half-block cells.
  ///
  /// Shells out to ffmpeg, which decodes the JPEG, scales it to the cell grid
  /// and hands back raw RGB. ffmpeg is built with TLS here, so it fetches the
  /// CDN URL itself and we need no HTTP client of our own.
  ///
  /// Best-effort by design: a missing ffmpeg, a dead URL or a slow CDN must
  /// never surface as a UI error, because this runs on ordinary cursor
  /// movement. Failures are logged and leave the pane empty.
  async fn refresh_playlist_cover(&self) -> Result<()> {
    // Decide what (if anything) to render while holding the lock, then drop it
    // before the await — ffmpeg takes tens of milliseconds and the UI thread
    // redraws off this same mutex.
    let target = {
      let s = self.state.lock().unwrap();
      if s.cover_render_disabled {
        return Ok(());
      }
      let Some(playlist) = s.playlists.get(s.playlists_index) else {
        return Ok(());
      };
      let id = playlist.id.id().to_string();
      // Already resolved for this playlist — this is what makes a burst of
      // queued refreshes collapse instead of re-spawning ffmpeg per keypress.
      if s
        .playlist_cover
        .as_ref()
        .is_some_and(|c| c.playlist_id == id)
      {
        return Ok(());
      }
      smallest_image_url(&playlist.images).map(|url| (id, url))
    };

    let Some((playlist_id, url)) = target else {
      // No image on this playlist. Record that so we don't look again.
      let mut s = self.state.lock().unwrap();
      if let Some(playlist) = s.playlists.get(s.playlists_index) {
        let id = playlist.id.id().to_string();
        s.playlist_cover = Some(PlaylistCover {
          playlist_id: id,
          art: None,
        });
      }
      return Ok(());
    };

    let key = cover_cache_key(&url);

    // Tier 1: already rendered this session. Costs a hash lookup, so scrolling
    // back over playlists you have already visited never touches ffmpeg.
    if let Some(art) = {
      let s = self.state.lock().unwrap();
      s.cover_cache.get(&key).cloned()
    } {
      self.publish_cover(&playlist_id, Some(art));
      return Ok(());
    }

    // Tier 2: rendered by an earlier run. Under 2 KB per read, so this is far
    // cheaper than a CDN fetch plus an ffmpeg spawn on every startup.
    if let Some(art) = read_cached_cover(&key).await {
      self
        .state
        .lock()
        .unwrap()
        .cover_cache
        .insert(key, art.clone());
      self.publish_cover(&playlist_id, Some(art));
      return Ok(());
    }

    let art = match render_cover(&url).await {
      Ok(art) => {
        // Best effort: a cache we cannot write is a slow cache, not an error.
        if let Err(err) = write_cached_cover(&key, &art).await {
          warn!(?err, "could not persist cover cache entry");
        }
        self
          .state
          .lock()
          .unwrap()
          .cover_cache
          .insert(key, art.clone());
        Some(art)
      }
      Err(err) => {
        if is_ffmpeg_missing(&err) {
          warn!(
            ?err,
            "ffmpeg unavailable — disabling cover art for this run"
          );
          self.state.lock().unwrap().cover_render_disabled = true;
          return Ok(());
        }
        warn!(?err, url = %url, "cover render failed");
        None
      }
    };

    self.publish_cover(&playlist_id, art);
    Ok(())
  }

  /// Publish a cover, but only if the cursor has not moved on to a different
  /// playlist while we were fetching or rendering it.
  fn publish_cover(&self, playlist_id: &str, art: Option<CoverArt>) {
    let mut s = self.state.lock().unwrap();
    // The selection may have moved while ffmpeg ran. Only publish if the
    // playlist we rendered is still the one under the cursor.
    let still_selected = s
      .playlists
      .get(s.playlists_index)
      .is_some_and(|p| p.id.id() == playlist_id);
    if still_selected {
      s.playlist_cover = Some(PlaylistCover {
        playlist_id: playlist_id.to_string(),
        art,
      });
    }
  }

  async fn get_playlist_tracks(&self, playlist_id: &str, playlist_name: &str) -> Result<()> {
    // Spotify restricted `/playlists/{id}/items` for apps without extended
    // quota in Feb 2026 — even for the caller's own playlists. On failure we
    // put an explanation row in the track table and set `context_uri` so
    // "play" on the TrackTable still kicks the whole playlist via Spotify
    // Connect (which doesn't go through the items API).
    //
    // For accounts where the endpoint works, page through at the API max of
    // 50 items until we get a short page or hit a safety bound (10k tracks
    // covers Spotify's per-playlist hard limit).
    const PAGE_LIMIT: u32 = 50;
    const MAX_TRACKS: usize = 10_000;
    let mut tracks: Vec<TrackRow> = Vec::new();
    let mut offset: u32 = 0;
    let mut first_error: Option<anyhow::Error> = None;
    loop {
      let pid = PlaylistId::from_id(playlist_id).context("invalid playlist id")?;
      let page_result = self
        .spotify
        .playlist_items_manual(pid, None, None, Some(PAGE_LIMIT), Some(offset))
        .await;
      match page_result {
        Ok(page) => {
          // Map (not filter) so local indices stay 1:1 with Spotify's playlist
          // positions. Episodes, removed/local tracks and Unknown items still
          // occupy a slot so the offset we pass to `start_context_playback`
          // matches what the user is looking at. Otherwise the penultimate
          // visible track plays the last one, etc.
          let next = page.next.clone();
          #[allow(deprecated)]
          tracks.extend(page.items.into_iter().map(|pi| match pi.item {
            Some(PlayableItem::Track(t)) => TrackRow::from_full(t),
            Some(PlayableItem::Episode(e)) => {
              use rspotify::prelude::Id;
              TrackRow {
                uri: Some(e.id.uri()),
                name: e.name,
                artists: e.show.name,
                album: String::new(),
                duration_ms: e.duration.num_milliseconds().max(0) as u64,
              }
            }
            Some(PlayableItem::Unknown(_)) | None => TrackRow {
              uri: None,
              name: "(unavailable)".to_string(),
              artists: String::new(),
              album: String::new(),
              duration_ms: 0,
            },
          }));
          // Spotify can return fewer items than `limit` even when more pages
          // exist (server-side filtering). The authoritative end signal is
          // `page.next == None`, not a short page.
          if next.is_none() || tracks.len() >= MAX_TRACKS {
            break;
          }
          offset += PAGE_LIMIT;
        }
        Err(err) => {
          first_error = Some(anyhow::Error::from(err));
          break;
        }
      }
    }

    let context_uri = format!("spotify:playlist:{playlist_id}");
    let mut state = self.state.lock().unwrap();
    state.track_list_title = playlist_name.to_string();
    state.track_list_context_uri = Some(context_uri);
    state.track_list_index = 0;

    if let Some(err) = first_error {
      if tracks.is_empty() {
        warn!(
          ?err,
          "playlist_items blocked — falling back to context-play only"
        );
        state.track_list = vec![TrackRow {
          uri: None,
          name: "(track listing unavailable — Spotify API restriction)".to_string(),
          artists: "press Enter to play the playlist via Spotify Connect".to_string(),
          album: String::new(),
          duration_ms: 0,
        }];
        state.last_error = Some(
          "playlist tracks blocked (403); Enter still starts playlist playback — apply for Spotify API extended quota to unlock listings"
            .to_string(),
        );
      } else {
        // Partial success: keep what we got and surface a short error.
        warn!(?err, "playlist_items pagination failed mid-stream");
        state.track_list = tracks;
        state.last_error = Some(format!("playlist tracks truncated: {err:#}"));
      }
    } else {
      state.track_list = tracks;
    }
    Ok(())
  }

  async fn get_saved_tracks(&self) -> Result<()> {
    let page = self
      .spotify
      .current_user_saved_tracks_manual(None, Some(50), None)
      .await?;
    let tracks = page
      .items
      .into_iter()
      .map(|st| TrackRow::from_full(st.track))
      .collect();
    let mut state = self.state.lock().unwrap();
    state.track_list = tracks;
    state.track_list_title = "Liked Songs".to_string();
    state.track_list_context_uri = None;
    state.track_list_index = 0;
    Ok(())
  }

  async fn get_album_tracks(&self, album_id: &str, album_name: &str) -> Result<()> {
    // Spotify's `/albums/{id}/tracks` caps `limit` at 50 per page. Page through
    // until we get a short page or hit the safety bound. 500 covers every
    // commercial album we'll plausibly see (longest known is ~120 tracks);
    // bumping the cap is cheap if it ever bites.
    const PAGE_LIMIT: u32 = 50;
    const MAX_TRACKS: usize = 500;
    let mut tracks: Vec<TrackRow> = Vec::new();
    let mut offset: u32 = 0;
    loop {
      let aid = AlbumId::from_id(album_id).context("invalid album id")?;
      let page = self
        .spotify
        .album_track_manual(aid, None, Some(PAGE_LIMIT), Some(offset))
        .await?;
      let got = page.items.len() as u32;
      tracks.extend(
        page
          .items
          .into_iter()
          .map(|st| TrackRow::from_simplified(st, album_name)),
      );
      if got < PAGE_LIMIT || tracks.len() >= MAX_TRACKS {
        break;
      }
      offset += PAGE_LIMIT;
    }
    let context_uri = format!("spotify:album:{album_id}");
    let mut state = self.state.lock().unwrap();
    state.track_list = tracks;
    state.track_list_title = format!("Album — {album_name}");
    state.track_list_context_uri = Some(context_uri);
    state.track_list_index = 0;
    Ok(())
  }

  /// Build an artist track list combining Spotify's curated top-tracks (when
  /// available) with paginated search results, deduped by track id. The
  /// curated endpoint is deprecated and capped at ~10 even when it works, so
  /// the search supplement keeps the list useful regardless of account quota.
  ///
  /// Returns `(rows, used_fallback_only)` — `used_fallback_only` is true when
  /// the curated endpoint failed (403/404) so the caller can label the list.
  async fn fetch_artist_tracks(
    &self,
    artist_id: &str,
    artist_name: &str,
  ) -> Result<(Vec<TrackRow>, bool)> {
    use rspotify::prelude::Id;
    let aid = ArtistId::from_id(artist_id).context("invalid artist id")?;
    let mut rows: Vec<TrackRow> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    #[allow(deprecated)]
    let curated = self.spotify.artist_top_tracks(aid.as_ref(), None).await;
    let curated_ok = curated.is_ok();
    if let Ok(tracks) = curated {
      for t in tracks {
        if let Some(id) = t.id.as_ref() {
          seen.insert(id.id().to_string());
        }
        rows.push(TrackRow::from_full(t));
      }
    }

    // Spotify's 2026-02-11 migration capped search `limit` at 10, so we page
    // up to 5 times to cover ~50 tracks for the artist.
    const LIMIT: u32 = 10;
    const PAGES: u32 = 5;
    let q = format!("artist:\"{artist_name}\"");
    for page_idx in 0..PAGES {
      let offset = page_idx * LIMIT;
      let result = self
        .spotify
        .search(&q, SearchType::Track, None, None, Some(LIMIT), Some(offset))
        .await;
      let (items, short) = match result {
        Ok(SearchResult::Tracks(p)) => {
          let short = (p.items.len() as u32) < LIMIT;
          (p.items, short)
        }
        Ok(_) => (Vec::new(), true),
        Err(err) => {
          warn!(?err, "artist track search page failed — stopping");
          break;
        }
      };
      for t in items {
        let new_id = t
          .id
          .as_ref()
          .map(|id| seen.insert(id.id().to_string()))
          .unwrap_or(true);
        if new_id {
          rows.push(TrackRow::from_full(t));
        }
      }
      if short {
        break;
      }
    }

    Ok((rows, !curated_ok))
  }

  async fn fetch_artist_albums(&self, artist_id: &str) -> Result<Vec<SimplifiedAlbum>> {
    // `/artists/{id}/albums` survived Spotify's 2024-11-27 deprecation pass,
    // but the endpoint can return 400 if `include_groups` is omitted on some
    // accounts post-2026-02-11 — pass the canonical full set explicitly.
    // `Market::FromToken` infers the user's country from the OAuth token so
    // we don't have to know it.
    const PAGE_LIMIT: u32 = 50;
    const MAX_ALBUMS: usize = 200;
    let groups = [
      AlbumType::Album,
      AlbumType::Single,
      AlbumType::Compilation,
      AlbumType::AppearsOn,
    ];
    let mut albums: Vec<SimplifiedAlbum> = Vec::new();
    let mut offset: u32 = 0;
    loop {
      let aid = ArtistId::from_id(artist_id).context("invalid artist id")?;
      let page = self
        .spotify
        .artist_albums_manual(
          aid,
          groups,
          Some(Market::FromToken),
          Some(PAGE_LIMIT),
          Some(offset),
        )
        .await?;
      let got = page.items.len() as u32;
      albums.extend(page.items);
      if got < PAGE_LIMIT || albums.len() >= MAX_ALBUMS {
        break;
      }
      offset += PAGE_LIMIT;
    }
    Ok(albums)
  }

  async fn open_artist(&self, artist_id: &str, artist_name: &str) -> Result<()> {
    // Reset the artist view first so a stale list doesn't flash while the
    // network calls are in flight.
    {
      let mut s = self.state.lock().unwrap();
      s.artist_view.artist_id = artist_id.to_string();
      s.artist_view.artist_name = artist_name.to_string();
      s.artist_view.tracks.clear();
      s.artist_view.albums.clear();
      s.artist_view.tracks_index = 0;
      s.artist_view.tracks_offset = 0;
      s.artist_view.albums_index = 0;
      s.artist_view.albums_offset = 0;
      s.artist_view.tab = crate::app::ArtistTab::Tracks;
    }

    // Run both fetches independently so a 4xx on one doesn't blank the other.
    let tracks_res = self.fetch_artist_tracks(artist_id, artist_name).await;
    let albums_res = self.fetch_artist_albums(artist_id).await;

    let tracks_err = tracks_res.as_ref().err().map(|e| format!("{e:#}"));
    let albums_err = albums_res.as_ref().err().map(|e| format!("{e:#}"));

    {
      let mut s = self.state.lock().unwrap();
      if let Ok((rows, _)) = tracks_res {
        s.artist_view.tracks = rows;
      }
      if let Ok(albums) = albums_res {
        s.artist_view.albums = albums;
      }
    }

    match (tracks_err, albums_err) {
      (None, None) => Ok(()),
      (Some(e), None) => anyhow::bail!("artist tracks: {e}"),
      (None, Some(e)) => anyhow::bail!("artist albums: {e}"),
      (Some(t), Some(a)) => anyhow::bail!("artist tracks: {t}; albums: {a}"),
    }
  }

  async fn search(&self, query: &str) -> Result<()> {
    // Each type is fetched independently so one failing endpoint doesn't kill
    // the whole search. Playlist search is dropped entirely — Spotify's
    // Feb 2026 migration makes `search?type=playlist` return 400/empty for
    // apps without extended quota, and even if it returned data we can't
    // drill into the results (items endpoint is also blocked).
    let types = [SearchType::Track, SearchType::Album, SearchType::Artist];
    let mut tracks = Vec::new();
    let mut albums = Vec::new();
    let mut artists = Vec::new();
    // Spotify's 2026-02-11 migration capped `limit` at 10 (previously 50, default
    // 20). We page internally so each tab still surfaces up to SEARCH_PAGES×LIMIT
    // results. Stops early when a page comes back short (i.e. no more results).
    const LIMIT: u32 = 10;
    const PAGES: u32 = 2;
    let mut sub_errors: Vec<String> = Vec::new();
    for t in types {
      for page_idx in 0..PAGES {
        let offset = page_idx * LIMIT;
        // rspotify 0.16's `FullArtist` still requires the `genres` field, but
        // Spotify stopped sending it alongside `popularity` and `followers` on
        // 2024-11-27. For the Artist sub-query we bypass rspotify's built-in
        // deserializer so we can patch missing `genres` with `[]` before parsing.
        let result = if matches!(t, SearchType::Artist) {
          self
            .search_artists_patched(query, LIMIT, offset)
            .await
            .map(|items| {
              // Synthesize a SearchResult::Artists so the match arms stay uniform.
              use rspotify::model::Page;
              SearchResult::Artists(Page {
                items,
                href: String::new(),
                limit: LIMIT,
                next: None,
                offset,
                previous: None,
                total: 0,
              })
            })
        } else {
          self
            .spotify
            .search(query, t, None, None, Some(LIMIT), Some(offset))
            .await
            .map_err(anyhow::Error::from)
        };
        let short_page = match result {
          Ok(SearchResult::Tracks(p)) => {
            let short = (p.items.len() as u32) < LIMIT;
            tracks.extend(p.items);
            short
          }
          Ok(SearchResult::Albums(p)) => {
            let short = (p.items.len() as u32) < LIMIT;
            albums.extend(p.items);
            short
          }
          Ok(SearchResult::Artists(p)) => {
            let short = (p.items.len() as u32) < LIMIT;
            artists.extend(p.items);
            short
          }
          Ok(_) => true,
          Err(err) => {
            warn!(?err, r#type = ?t, offset, "search sub-query failed — skipping");
            // Only surface first-page errors — later pages failing is usually
            // "end of results" and not worth blaring at the user.
            if page_idx == 0 {
              sub_errors.push(format!("{t:?}: {err}"));
            }
            true
          }
        };
        if short_page {
          break;
        }
      }
    }
    // Surface per-type failures in the error line so we can see which
    // sub-queries died. Cleared on the next successful network call.
    if !sub_errors.is_empty() {
      self.set_error(format!("search: {}", sub_errors.join(" | ")));
    }
    let mut state = self.state.lock().unwrap();
    state.search_results.tracks = tracks;
    state.search_results.albums = albums;
    state.search_results.artists = artists;
    state.search_results.tracks_index = 0;
    state.search_results.albums_index = 0;
    state.search_results.artists_index = 0;
    state.has_searched = true;
    Ok(())
  }

  async fn get_saved_albums(&self) -> Result<()> {
    let page = self
      .spotify
      .current_user_saved_albums_manual(None, Some(50), None)
      .await?;
    let mut state = self.state.lock().unwrap();
    state.saved_albums = page.items;
    state.saved_albums_index = 0;
    Ok(())
  }

  async fn get_followed_artists(&self) -> Result<()> {
    let page = self
      .spotify
      .current_user_followed_artists(None, Some(50))
      .await?;
    let mut state = self.state.lock().unwrap();
    state.followed_artists = page.items;
    state.followed_artists_index = 0;
    Ok(())
  }

  async fn get_recently_played(&self) -> Result<()> {
    let page = self
      .spotify
      .current_user_recently_played(Some(50), None)
      .await?;
    // De-dupe by track id while preserving order — recently played often repeats.
    let mut seen = HashSet::new();
    let tracks: Vec<TrackRow> = page
      .items
      .into_iter()
      .map(|h| h.track)
      .filter(|t| {
        use rspotify::prelude::Id;
        match t.id.as_ref() {
          Some(id) => seen.insert(id.id().to_string()),
          None => true,
        }
      })
      .map(TrackRow::from_full)
      .collect();
    let mut state = self.state.lock().unwrap();
    state.track_list = tracks;
    state.track_list_title = "Recently Played".to_string();
    state.track_list_context_uri = None;
    state.track_list_index = 0;
    Ok(())
  }

  async fn get_saved_shows(&self) -> Result<()> {
    let page = self.spotify.get_saved_show_manual(Some(50), None).await?;
    let mut state = self.state.lock().unwrap();
    state.saved_shows = page.items;
    state.saved_shows_index = 0;
    Ok(())
  }

  async fn get_show_episodes(&self, show_id: &str, show_name: &str) -> Result<()> {
    let sid = ShowId::from_id(show_id).context("invalid show id")?;
    let page = self
      .spotify
      .get_shows_episodes_manual(sid, None, Some(50), None)
      .await?;
    let mut state = self.state.lock().unwrap();
    state.show_episodes = page.items;
    state.show_episodes_title = show_name.to_string();
    state.show_episodes_index = 0;
    Ok(())
  }

  async fn get_queue(&self) -> Result<()> {
    let queue = self.spotify.current_user_queue().await?;
    let mut state = self.state.lock().unwrap();
    state.queue_current = queue.currently_playing;
    state.queue_items = queue.queue;
    state.queue_index = 0;
    Ok(())
  }

  async fn add_to_queue(&self, uri: &str) -> Result<()> {
    let device_id = self.resolve_play_device().await?;
    let playable = parse_playable_uri(uri)?;
    self
      .spotify
      .add_item_to_queue(playable, device_id.as_deref())
      .await?;
    Ok(())
  }

  async fn play_single_uri(&self, uri: &str) -> Result<()> {
    let device_id = self.resolve_play_device().await?;
    let playable = parse_playable_uri(uri)?;
    self
      .spotify
      .start_uris_playback([playable], device_id.as_deref(), None, None)
      .await?;
    self.refetch_after_mutation().await
  }

  async fn toggle_save_track(&self, track_id: &str) -> Result<()> {
    let tid = TrackId::from_id(track_id).context("invalid track id")?;
    let contained = self
      .spotify
      .library_contains([LibraryId::Track(tid.as_ref())])
      .await?;
    let is_saved = contained.into_iter().next().unwrap_or(false);
    if is_saved {
      self
        .spotify
        .library_remove([LibraryId::Track(tid.as_ref())])
        .await?;
    } else {
      self
        .spotify
        .library_add([LibraryId::Track(tid.as_ref())])
        .await?;
    }
    Ok(())
  }

  async fn toggle_save_album(&self, album_id: &str) -> Result<()> {
    let aid = AlbumId::from_id(album_id).context("invalid album id")?;
    let contained = self
      .spotify
      .library_contains([LibraryId::Album(aid.as_ref())])
      .await?;
    let is_saved = contained.into_iter().next().unwrap_or(false);
    if is_saved {
      self
        .spotify
        .library_remove([LibraryId::Album(aid.as_ref())])
        .await?;
    } else {
      self
        .spotify
        .library_add([LibraryId::Album(aid.as_ref())])
        .await?;
    }
    Ok(())
  }

  async fn toggle_follow_artist(&self, artist_id: &str) -> Result<()> {
    let aid = ArtistId::from_id(artist_id).context("invalid artist id")?;
    let contained = self
      .spotify
      .library_contains([LibraryId::Artist(aid.as_ref())])
      .await?;
    let is_followed = contained.into_iter().next().unwrap_or(false);
    if is_followed {
      self
        .spotify
        .library_remove([LibraryId::Artist(aid.as_ref())])
        .await?;
    } else {
      self
        .spotify
        .library_add([LibraryId::Artist(aid.as_ref())])
        .await?;
    }
    Ok(())
  }

  async fn unfollow_playlist(&self, playlist_id: &str) -> Result<()> {
    let pid = PlaylistId::from_id(playlist_id).context("invalid playlist id")?;
    self
      .spotify
      .library_remove([LibraryId::Playlist(pid.as_ref())])
      .await?;
    self.get_playlists().await
  }

  async fn get_devices(&self) -> Result<()> {
    let devices = self.spotify.device().await?;
    let mut state = self.state.lock().unwrap();
    state.devices = devices;
    state.devices_index = 0;
    Ok(())
  }

  async fn transfer_playback(&self, device_id: &str) -> Result<()> {
    self
      .spotify
      .transfer_playback(device_id, Some(true))
      .await?;
    self.refetch_after_mutation().await
  }

  async fn pause_playback(&self) -> Result<()> {
    let device_id = self.cached_device_id();
    self.spotify.pause_playback(device_id.as_deref()).await?;
    self.refetch_after_mutation().await
  }

  async fn resume_playback(&self) -> Result<()> {
    let device_id = self.resolve_play_device().await?;
    self
      .spotify
      .resume_playback(device_id.as_deref(), None)
      .await?;
    self.refetch_after_mutation().await
  }

  async fn next_track(&self) -> Result<()> {
    let device_id = self.cached_device_id();
    self.spotify.next_track(device_id.as_deref()).await?;
    self.refetch_after_mutation().await
  }

  async fn previous_track(&self) -> Result<()> {
    let device_id = self.cached_device_id();
    self.spotify.previous_track(device_id.as_deref()).await?;
    self.refetch_after_mutation().await
  }

  async fn change_volume(&self, volume_percent: u8) -> Result<()> {
    let device_id = self.cached_device_id();
    self
      .spotify
      .volume(volume_percent.min(100), device_id.as_deref())
      .await?;
    self.refetch_after_mutation().await
  }

  async fn seek(&self, position_ms: i64) -> Result<()> {
    let device_id = self.cached_device_id();
    self
      .spotify
      .seek_track(
        chrono::Duration::milliseconds(position_ms.max(0)),
        device_id.as_deref(),
      )
      .await?;
    self.refetch_after_mutation().await
  }

  async fn play_context(
    &self,
    context_uri: &str,
    offset_index: usize,
    track_uri: Option<&str>,
  ) -> Result<()> {
    let device_id = self.resolve_play_device().await?;
    let ctx = parse_context_uri(context_uri)?;
    // Prefer URI offset: Spotify locates the track in the context by URI, so
    // it works even when the playlist has slots Spotify doesn't index (local
    // tracks, market-restricted items). Fall back to positional offset for
    // placeholder rows that have no URI.
    let offset = match track_uri {
      Some(uri) => Offset::Uri(uri.to_string()),
      None => Offset::Position(chrono::Duration::milliseconds(offset_index as i64)),
    };
    self
      .spotify
      .start_context_playback(ctx, device_id.as_deref(), Some(offset), None)
      .await?;
    self.refetch_after_mutation().await
  }

  async fn play_uris(&self, uris: Vec<String>, offset_index: usize) -> Result<()> {
    let device_id = self.resolve_play_device().await?;
    let playable: Vec<PlayableId> = uris
      .iter()
      .filter_map(|u| TrackId::from_uri(u).ok().map(PlayableId::Track))
      .collect();
    if playable.is_empty() {
      anyhow::bail!("no playable URIs");
    }
    self
      .spotify
      .start_uris_playback(
        playable,
        device_id.as_deref(),
        Some(Offset::Position(chrono::Duration::milliseconds(
          offset_index as i64,
        ))),
        None,
      )
      .await?;
    self.refetch_after_mutation().await
  }

  fn cached_device_id(&self) -> Option<String> {
    self.state.lock().unwrap().device_id().map(String::from)
  }

  /// Pick a device for a playback-starting call.
  ///
  /// `cached_device_id()` only has a value when Spotify's `/me/player` returned
  /// a playback object — i.e. an *active* device. An official app that is
  /// merely running (no song started) shows up as a Connect device but is not
  /// active, so `current_playback` is `None`, we have no cached id, and
  /// Spotify's play endpoint then 404s with `NO_ACTIVE_DEVICE`.
  ///
  /// On a cache miss we list `/me/player/devices`, prefer any device flagged
  /// `is_active`, then fall back to the first non-restricted entry. If nothing
  /// comes back we return `None` and let Spotify surface its own error.
  async fn resolve_play_device(&self) -> Result<Option<String>> {
    if let Some(id) = self.cached_device_id() {
      return Ok(Some(id));
    }
    let devices = self.spotify.device().await?;
    let picked = devices
      .iter()
      .find(|d| d.is_active && !d.is_restricted)
      .or_else(|| devices.iter().find(|d| !d.is_restricted))
      .and_then(|d| d.id.clone());
    if picked.is_none() {
      anyhow::bail!(
        "no Spotify Connect device available — open the Spotify app (or spotifyd) on the account"
      );
    }
    Ok(picked)
  }

  async fn refetch_after_mutation(&self) -> Result<()> {
    tokio::time::sleep(Duration::from_millis(250)).await;
    self.get_current_playback().await
  }

  fn set_error(&self, msg: String) {
    let mut state = self.state.lock().unwrap();
    state.last_error = Some(msg);
    state.is_loading = false;
  }

  fn set_loading(&self, loading: bool) {
    self.state.lock().unwrap().is_loading = loading;
  }

  /// Artist-search workaround for rspotify 0.16's strict `FullArtist` schema.
  /// Calls the raw `/v1/search` endpoint via rspotify's `api_get`, walks the
  /// returned JSON, injects `"genres": []` into any artist object missing it,
  /// then parses through rspotify's deserializer. Drop this once rspotify
  /// gains `#[serde(default)]` on `FullArtist::genres`.
  async fn search_artists_patched(
    &self,
    query: &str,
    limit: u32,
    offset: u32,
  ) -> Result<Vec<rspotify::model::FullArtist>> {
    use rspotify::clients::BaseClient;
    use rspotify::http::Query;

    let limit_str = limit.to_string();
    let offset_str = offset.to_string();
    let mut params: Query = Query::new();
    params.insert("q", query);
    params.insert("type", "artist");
    params.insert("limit", &limit_str);
    params.insert("offset", &offset_str);
    let raw = self.spotify.api_get("search", &params).await?;

    let mut value: serde_json::Value = serde_json::from_str(&raw)?;
    if let Some(items) = value
      .get_mut("artists")
      .and_then(|a| a.get_mut("items"))
      .and_then(|i| i.as_array_mut())
    {
      for item in items {
        if item.get("genres").is_none() {
          if let Some(obj) = item.as_object_mut() {
            obj.insert("genres".into(), serde_json::Value::Array(Vec::new()));
          }
        }
      }
    }

    #[derive(serde::Deserialize)]
    struct ArtistsWrapper {
      artists: rspotify::model::Page<rspotify::model::FullArtist>,
    }
    let wrapper: ArtistsWrapper = serde_json::from_value(value)?;
    Ok(wrapper.artists.items)
  }
}

fn parse_playable_uri(uri: &str) -> Result<PlayableId<'static>> {
  if let Ok(id) = TrackId::from_uri(uri) {
    return Ok(PlayableId::Track(id.into_static()));
  }
  if let Ok(id) = EpisodeId::from_uri(uri) {
    return Ok(PlayableId::Episode(id.into_static()));
  }
  anyhow::bail!("unsupported playable URI: {uri}")
}

fn parse_context_uri(uri: &str) -> Result<PlayContextId<'static>> {
  if let Ok(id) = PlaylistId::from_uri(uri) {
    return Ok(PlayContextId::Playlist(id.into_static()));
  }
  if let Ok(id) = AlbumId::from_uri(uri) {
    return Ok(PlayContextId::Album(id.into_static()));
  }
  if let Ok(id) = ArtistId::from_uri(uri) {
    return Ok(PlayContextId::Artist(id.into_static()));
  }
  anyhow::bail!("unsupported context URI: {uri}")
}

/// Spotify returns playlist images largest-first. We only ever draw a
/// `COVER_COLS`-wide thumbnail, so take the smallest available and save the
/// bandwidth. `width` is nullable, so fall back to document order.
fn smallest_image_url(images: &[rspotify::model::Image]) -> Option<String> {
  images
    .iter()
    .min_by_key(|i| i.width.unwrap_or(u32::MAX))
    .or_else(|| images.last())
    .map(|i| i.url.clone())
}

fn is_ffmpeg_missing(err: &anyhow::Error) -> bool {
  err
    .downcast_ref::<std::io::Error>()
    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

/// Bytes per cell on disk: two RGB triples (top pixel, bottom pixel).
const CACHE_BYTES_PER_CELL: usize = 6;

/// Cache key for a cover image.
///
/// Spotify image URLs are content addressed — the final path segment changes
/// when the artwork changes. That makes it a correct invalidation key even
/// though the URL as a whole is documented as expiring within a day: a rotated
/// URL for unchanged artwork still hits, and changed artwork always misses.
fn cover_cache_key(url: &str) -> String {
  let path = url.split(['?', '#']).next().unwrap_or(url);
  let segment: String = path
    .rsplit('/')
    .next()
    .unwrap_or_default()
    .chars()
    .filter(|c| c.is_ascii_alphanumeric())
    .take(64)
    .collect();
  if !segment.is_empty() {
    return segment;
  }
  // Unrecognisable URL shape — fall back to hashing it. DefaultHasher is not
  // stable across Rust releases, which for a cache means an occasional miss
  // after a toolchain upgrade. That is acceptable; correctness does not
  // depend on it.
  use std::hash::{Hash, Hasher};
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  url.hash(&mut hasher);
  format!("h{:016x}", hasher.finish())
}

/// Cache entries carry their grid size in the filename, so changing
/// COVER_COLS or COVER_ROWS invalidates every old entry instead of loading
/// one at the wrong dimensions.
fn cover_cache_path(key: &str) -> Option<std::path::PathBuf> {
  let dir = crate::config::cover_cache_dir().ok()?;
  Some(dir.join(format!("{key}-{COVER_COLS}x{COVER_ROWS}.rgb")))
}

/// Cells are stored in cell order — 6 bytes each — so read and write are
/// exact mirrors and no framing or version header is needed.
async fn read_cached_cover(key: &str) -> Option<CoverArt> {
  let path = cover_cache_path(key)?;
  let bytes = tokio::fs::read(&path).await.ok()?;
  let expected = COVER_COLS as usize * COVER_ROWS as usize * CACHE_BYTES_PER_CELL;
  if bytes.len() != expected {
    // Truncated or half-written entry. Drop it and re-render.
    warn!(path = %path.display(), "discarding malformed cover cache entry");
    let _ = tokio::fs::remove_file(&path).await;
    return None;
  }
  let cells = bytes
    .chunks_exact(CACHE_BYTES_PER_CELL)
    .map(|c| ((c[0], c[1], c[2]), (c[3], c[4], c[5])))
    .collect();
  Some(CoverArt {
    cols: COVER_COLS,
    rows: COVER_ROWS,
    cells,
  })
}

async fn write_cached_cover(key: &str, art: &CoverArt) -> Result<()> {
  let path = cover_cache_path(key).context("resolving cover cache path")?;
  let mut bytes = Vec::with_capacity(art.cells.len() * CACHE_BYTES_PER_CELL);
  for ((tr, tg, tb), (br, bg, bb)) in &art.cells {
    bytes.extend_from_slice(&[*tr, *tg, *tb, *br, *bg, *bb]);
  }
  // Write to a temp file and rename, so a crash mid-write cannot leave a
  // short file that a later run would read as valid.
  let tmp = path.with_extension("rgb.tmp");
  tokio::fs::write(&tmp, &bytes)
    .await
    .with_context(|| format!("writing {}", tmp.display()))?;
  tokio::fs::rename(&tmp, &path)
    .await
    .with_context(|| format!("renaming into {}", path.display()))?;
  Ok(())
}

/// How long we give ffmpeg to fetch and decode before giving up. Generous for
/// a CDN round trip, short enough that a hung process can't wedge the network
/// task (which is serial — a stuck render would stall playback polling).
const COVER_TIMEOUT: Duration = Duration::from_secs(5);

async fn render_cover(url: &str) -> Result<CoverArt> {
  let cols = COVER_COLS;
  let rows = COVER_ROWS;
  let px_w = cols as u32;
  let px_h = rows as u32 * 2; // two stacked pixels per cell

  let mut cmd = tokio::process::Command::new("ffmpeg");
  cmd
    // ffmpeg reads stdin by default. The TUI owns stdin, so without this it
    // steals the user's keystrokes.
    .arg("-nostdin")
    .arg("-loglevel")
    .arg("error")
    // The URL is a Spotify API value passed as its own argv entry — never
    // through a shell — so there is nothing to quote or escape.
    .arg("-i")
    .arg(url)
    .arg("-vframes")
    .arg("1")
    .arg("-vf")
    .arg(format!("scale={px_w}:{px_h}:flags=lanczos"))
    .arg("-f")
    .arg("rawvideo")
    .arg("-pix_fmt")
    .arg("rgb24")
    .arg("-")
    .stdin(std::process::Stdio::null())
    .kill_on_drop(true);

  let output = tokio::time::timeout(COVER_TIMEOUT, cmd.output())
    .await
    .map_err(|_| anyhow::anyhow!("ffmpeg timed out after {COVER_TIMEOUT:?}"))?
    .map_err(anyhow::Error::from)
    .context("spawning ffmpeg")?;

  if !output.status.success() {
    anyhow::bail!(
      "ffmpeg exited {}: {}",
      output.status,
      String::from_utf8_lossy(output.stderr.as_slice()).trim()
    );
  }

  let want = (px_w * px_h * 3) as usize;
  if output.stdout.len() < want {
    anyhow::bail!(
      "ffmpeg returned {} bytes, expected {want}",
      output.stdout.len()
    );
  }

  let px = &output.stdout[..want];
  let at = |x: u32, y: u32| -> Rgb {
    let i = ((y * px_w + x) * 3) as usize;
    (px[i], px[i + 1], px[i + 2])
  };

  let mut cells = Vec::with_capacity((cols as usize) * (rows as usize));
  for row in 0..rows as u32 {
    for col in 0..cols as u32 {
      cells.push((at(col, row * 2), at(col, row * 2 + 1)));
    }
  }

  Ok(CoverArt { cols, rows, cells })
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  /// Writes a 48x48 PPM: top half pure red, bottom half pure blue.
  fn fixture(path: &std::path::Path) {
    let (w, h) = (48usize, 48usize);
    let mut buf = format!("P6\n{w} {h}\n255\n").into_bytes();
    for y in 0..h {
      let px: [u8; 3] = if y < h / 2 { [255, 0, 0] } else { [0, 0, 255] };
      for _ in 0..w {
        buf.extend_from_slice(&px);
      }
    }
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(&buf).unwrap();
  }

  fn have_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg")
      .arg("-version")
      .stdout(std::process::Stdio::null())
      .stderr(std::process::Stdio::null())
      .status()
      .is_ok()
  }

  /// The half-block encoding is easy to get subtly wrong — rows flipped,
  /// or fg/bg (top/bottom) swapped. A red-over-blue fixture catches both.
  #[tokio::test]
  async fn render_cover_preserves_orientation_and_channels() {
    if !have_ffmpeg() {
      eprintln!("skipping: ffmpeg not in PATH");
      return;
    }
    let dir = std::env::temp_dir().join("spotuify-cover-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fixture.ppm");
    fixture(&path);

    let art = render_cover(path.to_str().unwrap()).await.unwrap();

    assert_eq!(art.cols, COVER_COLS);
    assert_eq!(art.rows, COVER_ROWS);
    assert_eq!(
      art.cells.len(),
      COVER_COLS as usize * COVER_ROWS as usize,
      "one cell per grid position"
    );

    // First row: both stacked pixels land in the red half.
    let ((tr, _, tb), (br, _, bb)) = art.cells[0];
    assert!(tr > 200 && tb < 60, "top of first cell should be red");
    assert!(br > 200 && bb < 60, "bottom of first cell should be red");

    // Last row: both land in the blue half. If rows were flipped this fails.
    let last = art.cells.len() - COVER_COLS as usize;
    let ((tr, _, tb), (br, _, bb)) = art.cells[last];
    assert!(tb > 200 && tr < 60, "top of last row should be blue");
    assert!(bb > 200 && br < 60, "bottom of last row should be blue");
  }

  /// Same artwork must hit even when the URL's expiring query token rotates;
  /// different artwork must miss. Both directions matter: the first prevents
  /// pointless re-renders, the second prevents showing a stale cover.
  #[test]
  fn cache_key_tracks_artwork_not_url() {
    let a = "https://i.scdn.co/image/ab67706c0000da84aaaa1111";
    let rotated = "https://i.scdn.co/image/ab67706c0000da84aaaa1111?token=xyz&t=99";
    let different = "https://i.scdn.co/image/ab67706c0000da84bbbb2222";

    assert_eq!(cover_cache_key(a), cover_cache_key(rotated), "same artwork");
    assert_ne!(
      cover_cache_key(a),
      cover_cache_key(different),
      "different artwork"
    );
  }

  #[test]
  fn cache_key_is_filesystem_safe() {
    for url in [
      "https://i.scdn.co/image/../../etc/passwd",
      "https://i.scdn.co/image/",
      "not even a url",
      "",
    ] {
      let key = cover_cache_key(url);
      assert!(!key.is_empty(), "key for {url:?} must not be empty");
      assert!(
        key.chars().all(|c| c.is_ascii_alphanumeric()),
        "key for {url:?} must be alphanumeric, got {key:?}"
      );
    }
  }

  fn sample_art() -> CoverArt {
    let cells = (0..COVER_COLS as usize * COVER_ROWS as usize)
      .map(|i| {
        let n = i as u8;
        ((n, n.wrapping_add(1), n.wrapping_add(2)), (n, 0, 255 - n))
      })
      .collect();
    CoverArt {
      cols: COVER_COLS,
      rows: COVER_ROWS,
      cells,
    }
  }

  #[tokio::test]
  async fn cache_round_trip_is_lossless() {
    let art = sample_art();
    let key = "spotuifytestroundtrip";
    write_cached_cover(key, &art).await.unwrap();
    let back = read_cached_cover(key).await.expect("cache entry readable");

    assert_eq!(back.cols, art.cols);
    assert_eq!(back.rows, art.rows);
    assert_eq!(back.cells, art.cells, "cells must survive verbatim");

    let _ = tokio::fs::remove_file(cover_cache_path(key).unwrap()).await;
  }

  /// A truncated entry (crash mid-write, full disk) must be discarded rather
  /// than rendered as a partial cover.
  #[tokio::test]
  async fn cache_rejects_and_removes_truncated_entry() {
    let key = "spotuifytesttruncated";
    let path = cover_cache_path(key).unwrap();
    tokio::fs::write(&path, b"too short").await.unwrap();

    assert!(read_cached_cover(key).await.is_none(), "must not be used");
    assert!(!path.exists(), "malformed entry should be removed");
  }

  #[tokio::test]
  async fn cache_miss_returns_none() {
    assert!(read_cached_cover("spotuifytestdefinitelyabsent")
      .await
      .is_none());
  }

  #[test]
  fn smallest_image_url_prefers_narrowest() {
    use rspotify::model::Image;
    let images = vec![
      Image {
        height: Some(640),
        url: "big".into(),
        width: Some(640),
      },
      Image {
        height: Some(64),
        url: "small".into(),
        width: Some(64),
      },
      Image {
        height: Some(300),
        url: "mid".into(),
        width: Some(300),
      },
    ];
    assert_eq!(smallest_image_url(&images).as_deref(), Some("small"));
    assert_eq!(smallest_image_url(&[]), None);
  }

  /// Width is nullable in the API; unknown widths must not win the min().
  #[test]
  fn smallest_image_url_handles_null_width() {
    use rspotify::model::Image;
    let images = vec![
      Image {
        height: None,
        url: "unknown".into(),
        width: None,
      },
      Image {
        height: Some(64),
        url: "known".into(),
        width: Some(64),
      },
    ];
    assert_eq!(smallest_image_url(&images).as_deref(), Some("known"));
  }
}
