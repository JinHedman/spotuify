/// Adjust `offset` so that `selected` stays within the visible window,
/// with a `margin` of rows kept above and below the selection when possible.
///
/// Symmetric — both up and down edges trigger a 1-row scroll `margin` rows
/// before the selection hits the viewport boundary.
///
/// `item_count` is required to clamp the window to the end of the list. The
/// bottom margin exists to reveal rows *below* the selection; once the last
/// item is on screen there is nothing left to reveal, and applying the margin
/// anyway scrolls `margin` rows past the end — which renders as blank rows at
/// the bottom of the pane.
pub fn adjust_offset(
  selected: usize,
  offset: &mut usize,
  visible_height: usize,
  margin: usize,
  item_count: usize,
) {
  if visible_height == 0 || item_count == 0 {
    *offset = 0;
    return;
  }
  let margin = margin.min(visible_height / 2);

  // Top edge: keep at least `margin` rows above selection when possible.
  let max_offset_keeping_top_margin = selected.saturating_sub(margin);
  if *offset > max_offset_keeping_top_margin {
    *offset = max_offset_keeping_top_margin;
  }

  // Bottom edge: keep at least `margin` rows below selection when possible.
  let min_offset_keeping_bottom_margin = selected
    .saturating_add(margin + 1)
    .saturating_sub(visible_height);
  if *offset < min_offset_keeping_bottom_margin {
    *offset = min_offset_keeping_bottom_margin;
  }

  // Applied last so it wins over the bottom margin: never scroll past the
  // final item. Saturates to 0 when the list is shorter than the viewport.
  let max_offset = item_count.saturating_sub(visible_height);
  if *offset > max_offset {
    *offset = max_offset;
  }
}

/// Draw a position indicator down the right edge of `area`, but only when the
/// list actually overflows.
///
/// Always-visible scrollbars on short lists are noise; the useful signal is
/// "there is more than you can see", so absence carries meaning too.
///
/// `area` is the bordered pane. The bar is inset by one cell so it sits inside
/// the frame rather than overwriting it.
pub fn render(
  frame: &mut ratatui::Frame,
  area: ratatui::layout::Rect,
  offset: usize,
  visible: usize,
  item_count: usize,
  theme: &crate::config::theme::Theme,
) {
  use ratatui::layout::{Margin, Rect};
  use ratatui::style::Style;
  use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

  if item_count <= visible || visible == 0 || area.height < 3 || area.width < 3 {
    return;
  }

  let inner: Rect = area.inner(Margin {
    horizontal: 0,
    vertical: 1,
  });

  let mut scroll_state = ScrollbarState::new(item_count.saturating_sub(visible))
    .position(offset)
    .viewport_content_length(visible);

  let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
    .begin_symbol(None)
    .end_symbol(None)
    .track_symbol(None)
    .thumb_symbol("\u{2503}")
    .thumb_style(Style::default().fg(theme.inactive));

  frame.render_stateful_widget(bar, inner, &mut scroll_state);
}

#[cfg(test)]
mod tests {
  use super::adjust_offset;

  const MARGIN: usize = 2;

  fn offset_for(selected: usize, visible: usize, item_count: usize) -> usize {
    let mut offset = 0;
    // Walk the selection down from the top the way a user would, so the
    // result reflects accumulated state rather than a cold jump.
    for s in 0..=selected {
      adjust_offset(s, &mut offset, visible, MARGIN, item_count);
    }
    offset
  }

  /// The regression: selecting the last item used to leave `margin` blank
  /// rows at the bottom of the pane.
  #[test]
  fn last_item_does_not_scroll_past_the_end() {
    let (visible, count) = (10, 40);
    let offset = offset_for(count - 1, visible, count);
    assert_eq!(offset, count - visible, "window must end at the last item");
    assert_eq!(
      offset + visible,
      count,
      "no rows may be rendered beyond the list"
    );
  }

  #[test]
  fn short_list_never_scrolls() {
    let mut offset = 0;
    adjust_offset(2, &mut offset, 10, MARGIN, 3);
    assert_eq!(offset, 0);
  }

  #[test]
  fn empty_list_resets_offset() {
    let mut offset = 7;
    adjust_offset(0, &mut offset, 10, MARGIN, 0);
    assert_eq!(offset, 0);
  }

  #[test]
  fn zero_height_resets_offset() {
    let mut offset = 7;
    adjust_offset(5, &mut offset, 0, MARGIN, 40);
    assert_eq!(offset, 0);
  }

  /// Mid-list the margin still applies in both directions.
  #[test]
  fn keeps_margin_below_selection_mid_list() {
    let (visible, count) = (10, 40);
    let selected = 20;
    let offset = offset_for(selected, visible, count);
    assert!(
      selected >= offset && selected < offset + visible,
      "selection must be visible"
    );
    assert!(
      offset + visible - selected > MARGIN,
      "at least {MARGIN} rows below the selection"
    );
  }

  #[test]
  fn scrolling_back_up_keeps_margin_above() {
    let (visible, count) = (10, 40);
    let mut offset = 0;
    for s in 0..count {
      adjust_offset(s, &mut offset, visible, MARGIN, count);
    }
    // Now walk back up and check the top margin holds.
    for s in (0..count).rev() {
      adjust_offset(s, &mut offset, visible, MARGIN, count);
      assert!(s >= offset && s < offset + visible, "selection {s} visible");
      assert!(offset + visible <= count, "never past the end at {s}");
    }
    assert_eq!(offset, 0, "back at the top");
  }

  /// Every position in a list must be reachable with the selection on screen
  /// and the window inside bounds — the two invariants together.
  #[test]
  fn invariants_hold_across_sizes() {
    for count in [1usize, 2, 5, 13, 40, 101] {
      for visible in [1usize, 2, 3, 7, 10, 25] {
        let mut offset = 0;
        for s in 0..count {
          adjust_offset(s, &mut offset, visible, MARGIN, count);
          assert!(
            s >= offset && s < offset + visible,
            "count={count} visible={visible} selected={s}: selection off screen (offset={offset})"
          );
          assert!(
            offset + visible <= count.max(visible),
            "count={count} visible={visible} selected={s}: window past end (offset={offset})"
          );
        }
      }
    }
  }
}
