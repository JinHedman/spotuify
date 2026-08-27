use crate::app::AppState;
use ratatui::{
  layout::Rect,
  style::{Color, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Paragraph},
  Frame,
};
use rspotify::prelude::Id;

/// The upper-half-block glyph. Foreground paints the top pixel of the cell,
/// background the bottom one, giving two full-colour pixels per cell.
const HALF_BLOCK: &str = "\u{2580}";

pub fn draw(frame: &mut Frame, area: Rect, state: &AppState) {
  let theme = state.theme;
  let block = Block::new()
    .borders(Borders::ALL)
    .title(" Cover ")
    .border_style(Style::default().fg(theme.inactive));

  // Only draw art that belongs to the playlist currently under the cursor.
  // The network task publishes asynchronously, so a stale cover can briefly
  // outlive the selection that requested it.
  let selected_id = state
    .playlists
    .get(state.playlists_index)
    .map(|p| p.id.id().to_string());

  let art = state
    .playlist_cover
    .as_ref()
    .filter(|c| Some(&c.playlist_id) == selected_id.as_ref())
    .and_then(|c| c.art.as_ref());

  let Some(art) = art else {
    let hint = if state.cover_render_disabled {
      "(ffmpeg not found)"
    } else if selected_id.is_none() {
      ""
    } else {
      "(no cover)"
    };
    let placeholder = Paragraph::new(Line::styled(hint, Style::default().fg(theme.hint)))
      .centered()
      .block(block);
    frame.render_widget(placeholder, area);
    return;
  };

  let inner = block.inner(area);
  frame.render_widget(block, area);

  let lines: Vec<Line> = (0..art.rows as usize)
    .map(|row| {
      let spans: Vec<Span> = (0..art.cols as usize)
        .filter_map(|col| art.cells.get(row * art.cols as usize + col))
        .map(|&((tr, tg, tb), (br, bg, bb))| {
          Span::styled(
            HALF_BLOCK,
            Style::default()
              .fg(Color::Rgb(tr, tg, tb))
              .bg(Color::Rgb(br, bg, bb)),
          )
        })
        .collect();
      Line::from(spans)
    })
    .collect();

  // Centre the fixed-size art in whatever room the sidebar gave us. Clipping
  // is left to ratatui if the pane is narrower than the art.
  let art_width = art.cols.min(inner.width);
  let x_pad = inner.width.saturating_sub(art_width) / 2;
  let target = Rect {
    x: inner.x + x_pad,
    y: inner.y,
    width: art_width,
    height: art.rows.min(inner.height),
  };
  frame.render_widget(Paragraph::new(lines), target);
}
