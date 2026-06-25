/// Adjust `offset` so that `selected` stays within the visible window,
/// with a `margin` of rows kept above and below the selection when possible.
///
/// Symmetric — both up and down edges trigger a 1-row scroll `margin` rows
/// before the selection hits the viewport boundary.
pub fn adjust_offset(selected: usize, offset: &mut usize, visible_height: usize, margin: usize) {
  if visible_height == 0 {
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
}
