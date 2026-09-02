pub mod artist_view;
pub mod banner;
pub mod basic_view;
pub mod dialog;
pub mod followed_artists;
pub mod help;
pub mod layout;
pub mod legend;
pub mod library;
pub mod nowplaying;
pub mod playbar;
pub mod playlist_cover;
pub mod playlists;
pub mod queue;
pub mod saved_albums;
pub mod saved_shows;
pub mod scroll;
pub mod search_input;
pub mod search_results;
pub mod select_device;
pub mod show_episodes;
pub mod spinner;
pub mod theme_picker;
pub mod too_small;
pub mod track_table;

use crate::app::{ActiveBlock, AppState, COVER_ROWS};
use ratatui::{
  layout::{Constraint, Direction, Layout},
  Frame,
};
use std::sync::{Arc, Mutex};

/// Width below which the sidebar is hidden.
const NARROW_WIDTH: u16 = 110;
/// Height below which only the playbar is shown (basic view).
const BASIC_VIEW_HEIGHT: u16 = 18;
/// Below either dimension the UI is unusable — show a message instead.
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 10;
/// Total width required (banner + gap + search) before we render the header banner.
const BANNER_MIN_WIDTH: u16 = 80;
/// Width of the left column (sidebar below, banner above). Kept constant so
/// the banner lines up with the Library / Playlists boxes under it.
const SIDEBAR_WIDTH: u16 = 38;
/// Playlist rows that must survive before the cover box earns its space.
const MIN_PLAYLIST_ROWS: u16 = 8;
/// Cover art plus its border.
const COVER_HEIGHT: u16 = COVER_ROWS + 2;

pub fn draw(frame: &mut Frame, state: &Arc<Mutex<AppState>>) {
  let area = frame.area();
  let mut state = state.lock().unwrap();

  if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
    too_small::draw(frame, area, &state.theme);
    return;
  }

  if area.height < BASIC_VIEW_HEIGHT {
    basic_view::draw(frame, area, &state);
    return;
  }

  let show_sidebar = area.width >= NARROW_WIDTH;
  let show_banner = area.width >= BANNER_MIN_WIDTH;

  let showing_search = matches!(
    state.active_block,
    ActiveBlock::SearchInput | ActiveBlock::SearchResults
  ) || state.has_searched;

  let header_height = if show_banner {
    banner::BANNER_HEIGHT
  } else {
    3
  };

  let rows = Layout::new(
    Direction::Vertical,
    [
      Constraint::Length(header_height),
      Constraint::Min(1),
      Constraint::Length(4),
      Constraint::Length(1),
    ],
  )
  .split(area);

  if show_banner {
    let header_cols = Layout::new(
      Direction::Horizontal,
      [
        Constraint::Length(SIDEBAR_WIDTH),
        Constraint::Length(1),
        Constraint::Min(20),
      ],
    )
    .split(rows[0]);

    banner::draw(frame, header_cols[0], &state.theme);

    // Vertically center the 3-row search box inside the taller header.
    let search_area = Layout::new(
      Direction::Vertical,
      [
        Constraint::Length(header_cols[2].height.saturating_sub(3) / 2),
        Constraint::Length(3),
        Constraint::Min(0),
      ],
    )
    .split(header_cols[2])[1];
    search_input::draw(frame, search_area, &state);
  } else {
    search_input::draw(frame, rows[0], &state);
  }

  let content_area = if show_sidebar {
    let cols = Layout::new(
      Direction::Horizontal,
      [Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(1)],
    )
    .split(rows[1]);

    // The cover box is fixed height. Only give it room when the playlist list
    // can still show a useful number of entries, otherwise the art crowds out
    // the thing it annotates.
    let show_cover = cols[0].height >= 7 + COVER_HEIGHT + MIN_PLAYLIST_ROWS;
    let cover_height = if show_cover { COVER_HEIGHT } else { 0 };

    let sidebar = Layout::new(
      Direction::Vertical,
      [
        Constraint::Length(7),
        Constraint::Min(1),
        Constraint::Length(cover_height),
      ],
    )
    .split(cols[0]);

    library::draw(frame, sidebar[0], &state);
    playlists::draw(frame, sidebar[1], &mut state);
    if show_cover {
      playlist_cover::draw(frame, sidebar[2], &state);
    }
    cols[1]
  } else {
    rows[1]
  };

  match state.active_block {
    ActiveBlock::SearchInput | ActiveBlock::SearchResults if showing_search => {
      search_results::draw(frame, content_area, &state);
    }
    ActiveBlock::SavedAlbums => saved_albums::draw(frame, content_area, &mut state),
    ActiveBlock::FollowedArtists => followed_artists::draw(frame, content_area, &mut state),
    ActiveBlock::ArtistView => artist_view::draw(frame, content_area, &mut state),
    ActiveBlock::SavedShows => saved_shows::draw(frame, content_area, &mut state),
    ActiveBlock::ShowEpisodes => show_episodes::draw(frame, content_area, &mut state),
    _ => track_table::draw(frame, content_area, &mut state),
  }

  playbar::draw(frame, rows[2], &state);
  legend::draw(frame, rows[3], &state.theme);

  if state.active_block == ActiveBlock::SelectDevice {
    select_device::draw(frame, area, &state);
  }

  if state.active_block == ActiveBlock::Queue {
    queue::draw(frame, area, &state);
  }

  if state.active_block == ActiveBlock::Dialog {
    dialog::draw(frame, area, &state);
  }

  if state.active_block == ActiveBlock::ThemePicker {
    theme_picker::draw(frame, area, &state);
  }

  if state.help_visible {
    help::draw(frame, area, &mut state);
  }
}
