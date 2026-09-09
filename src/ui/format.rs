//! Shared formatting for values rendered in more than one pane.

/// `M:SS` for a duration in milliseconds.
///
/// Lives here because three panes need it — the playbar, the track table and
/// the search results — and a third private copy would have made the next
/// change to it a three-file edit.
///
/// Known limitation: no hour rollover, so a 96-minute podcast renders as
/// `96:00` rather than `1:36:00`. Left as-is here to keep this change to a
/// move; with one definition it is now a single-line fix.
pub fn ms(ms: u64) -> String {
  let total_secs = ms / 1000;
  let minutes = total_secs / 60;
  let seconds = total_secs % 60;
  format!("{minutes}:{seconds:02}")
}
