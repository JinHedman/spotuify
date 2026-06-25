#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveBlock {
  Library,
  MyPlaylists,
  TrackTable,
  SearchInput,
  SearchResults,
  SelectDevice,
  SavedAlbums,
  FollowedArtists,
  ArtistView,
  SavedShows,
  ShowEpisodes,
  Queue,
  Dialog,
  ThemePicker,
}

impl ActiveBlock {
  pub fn is_home(self) -> bool {
    matches!(self, Self::Library | Self::MyPlaylists | Self::TrackTable)
  }

  /// True if the block occupies the right-hand content pane (as opposed to
  /// the sidebar or a modal overlay). Used by directional navigation to know
  /// where "left" goes.
  pub fn is_content_pane(self) -> bool {
    matches!(
      self,
      Self::TrackTable
        | Self::SearchResults
        | Self::SavedAlbums
        | Self::FollowedArtists
        | Self::ArtistView
        | Self::SavedShows
        | Self::ShowEpisodes
    )
  }

  pub fn go_left(self) -> Self {
    if self.is_content_pane() {
      Self::MyPlaylists
    } else {
      self
    }
  }

  pub fn go_right(self) -> Self {
    match self {
      Self::Library | Self::MyPlaylists => Self::TrackTable,
      _ => self,
    }
  }

  pub fn go_up(self) -> Self {
    match self {
      Self::MyPlaylists => Self::Library,
      _ => self,
    }
  }

  pub fn go_down(self) -> Self {
    match self {
      Self::Library => Self::MyPlaylists,
      _ => self,
    }
  }
}

// Playlists intentionally absent — Spotify's Feb-2026 migration blocks
// `/search?type=playlist` results from returning anything useful for apps
// without extended quota, and drilling into a playlist's items is also blocked.
// Keeping the tab around would only surface a dead end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchTab {
  Tracks,
  Albums,
  Artists,
}

impl SearchTab {
  pub const ALL: [SearchTab; 3] = [Self::Tracks, Self::Albums, Self::Artists];

  pub fn index(self) -> usize {
    match self {
      Self::Tracks => 0,
      Self::Albums => 1,
      Self::Artists => 2,
    }
  }

  pub fn title(self) -> &'static str {
    match self {
      Self::Tracks => "Tracks",
      Self::Albums => "Albums",
      Self::Artists => "Artists",
    }
  }

  pub fn next(self) -> Self {
    match self {
      Self::Tracks => Self::Albums,
      Self::Albums => Self::Artists,
      Self::Artists => Self::Tracks,
    }
  }

  pub fn prev(self) -> Self {
    match self {
      Self::Tracks => Self::Artists,
      Self::Albums => Self::Tracks,
      Self::Artists => Self::Albums,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtistTab {
  Tracks,
  Albums,
}

impl ArtistTab {
  pub const ALL: [ArtistTab; 2] = [Self::Tracks, Self::Albums];

  pub fn index(self) -> usize {
    match self {
      Self::Tracks => 0,
      Self::Albums => 1,
    }
  }

  pub fn title(self) -> &'static str {
    match self {
      Self::Tracks => "Top tracks",
      Self::Albums => "Albums",
    }
  }

  pub fn next(self) -> Self {
    match self {
      Self::Tracks => Self::Albums,
      Self::Albums => Self::Tracks,
    }
  }

  pub fn prev(self) -> Self {
    self.next()
  }
}
