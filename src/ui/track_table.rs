use crate::app::{ActiveBlock, AppState};
use crate::ui::{layout, scroll};
use ratatui::{
  layout::{Constraint, Rect},
  style::{Modifier, Style},
  widgets::{Cell, Paragraph, Row, Table, TableState},
  Frame,
};

const SCROLL_MARGIN: usize = 2;

pub fn draw(frame: &mut Frame, area: Rect, state: &mut AppState) {
  let theme = state.theme;
  let title = if state.track_list_title.is_empty() {
    "Tracks".to_string()
  } else {
    format!("Tracks — {}", state.track_list_title)
  };
  // A fetch replaces the list only once it completes, so while switching from
  // one playlist to another the previous tracks stay on screen. Marking the
  // title covers that case, where an empty-state spinner never fires.
  let title = if state.track_list_loading {
    format!(
      "{title}  {}",
      crate::ui::spinner::frame(crate::ui::spinner::now_ms())
    )
  } else {
    title
  };
  let block = layout::block(&title, ActiveBlock::TrackTable, state.active_block, &theme);

  if state.track_list.is_empty() {
    let placeholder = if state.track_list_loading {
      // Paging a large playlist is many round trips; the idle hint below would
      // otherwise sit there looking like nothing had happened.
      Paragraph::new(crate::ui::spinner::line("loading tracks…", &theme))
    } else {
      Paragraph::new("Pick a playlist, search with /, or open Liked Songs.")
    };
    frame.render_widget(placeholder.block(block), area);
    return;
  }

  let header = Row::new(vec![
    Cell::from("#"),
    Cell::from("Title"),
    Cell::from("Artist"),
    Cell::from("Album"),
    Cell::from("Time"),
  ])
  .style(Style::default().fg(theme.hint).add_modifier(Modifier::BOLD));

  // Matched on URI rather than on list position, because Spotify reports what
  // is playing but not where in the context it sits. Consequence: a track
  // appearing twice in a playlist marks both rows. That is not fixable from
  // the API, and showing both is more honest than guessing one.
  //
  // Deliberately not requiring the context to match as well: Liked Songs,
  // search results and Recently Played set no `track_list_context_uri`, so a
  // context check would hide the marker in exactly the views that most need
  // it. The claim here is "this is the song that's playing", not "playback is
  // running from this list".
  let playing_uri = state.playing_uri();
  let is_playing = state.is_playing();
  let anim_ms = crate::ui::spinner::now_ms();

  let rows: Vec<Row> = state
    .track_list
    .iter()
    .enumerate()
    .map(|(i, t)| {
      let current = crate::ui::nowplaying::is_current(t.uri.as_deref(), playing_uri.as_deref());
      let gutter = if current {
        crate::ui::nowplaying::glyph(anim_ms, is_playing).to_string()
      } else {
        // Same width as the glyph, so the column does not shift when the
        // marker appears or moves.
        format!("{:>width$}", i + 1, width = crate::ui::nowplaying::WIDTH)
      };
      let row = Row::new(vec![
        Cell::from(gutter),
        Cell::from(t.name.clone()),
        Cell::from(t.artists.clone()),
        Cell::from(t.album.clone()),
        Cell::from(format_ms(t.duration_ms)),
      ]);
      if current {
        // Foreground only, so it composes with the selection background when
        // the playing row also happens to be selected.
        row.style(Style::default().fg(theme.playing_icon))
      } else {
        row
      }
    })
    .collect();

  let widths = [
    Constraint::Length(4),
    Constraint::Percentage(32),
    Constraint::Percentage(28),
    Constraint::Percentage(28),
    Constraint::Length(6),
  ];

  let table = Table::new(rows, widths)
    .header(header)
    .block(block)
    .row_highlight_style(
      Style::default()
        .bg(theme.selected_bg)
        .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

  let visible = (area.height as usize).saturating_sub(3);
  scroll::adjust_offset(
    state.track_list_index,
    &mut state.track_list_offset,
    visible,
    SCROLL_MARGIN,
    state.track_list.len(),
  );

  let mut table_state = TableState::default();
  if state.active_block == ActiveBlock::TrackTable {
    table_state.select(Some(state.track_list_index));
  }
  *table_state.offset_mut() = state.track_list_offset;
  frame.render_stateful_widget(table, area, &mut table_state);
}

fn format_ms(ms: u64) -> String {
  let total_secs = ms / 1000;
  let minutes = total_secs / 60;
  let seconds = total_secs % 60;
  format!("{minutes}:{seconds:02}")
}
