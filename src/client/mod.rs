use crate::app::{AppState, TrackRow};
use anyhow::{Context, Result};
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
    let page = self
      .spotify
      .current_user_playlists_manual(Some(50), None)
      .await?;
    let mut state = self.state.lock().unwrap();
    state.playlists = page.items;
    state.playlists_index = state
      .playlists_index
      .min(state.playlists.len().saturating_sub(1));
    Ok(())
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
