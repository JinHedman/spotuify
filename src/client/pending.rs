//! One-slot dedupe for idempotent `IoEvent`s.
//!
//! The network task handles events one at a time, so a slow request (a
//! network drop, an ffmpeg cover render) lets the channel fill. For events
//! that carry no data and re-read state when dequeued — the playback poll and
//! the playlist cover refresh — a second queued copy does nothing the first
//! would not, so at most one is ever queued. Sending with `try_send` means the
//! caller never waits on a full channel either: the UI loop must keep drawing
//! and reading keys no matter how far behind the network task is.

use super::IoEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::{self, error::TrySendError};

/// "An event of this kind is queued and not yet dequeued."
#[derive(Debug, Default)]
pub struct PendingFlag(AtomicBool);

impl PendingFlag {
  /// Marks the event as queued. Returns `false` if one already was, in which
  /// case the caller must not send another.
  pub fn claim(&self) -> bool {
    !self.0.swap(true, Ordering::AcqRel)
  }

  /// Called by the network task when it dequeues the event, *before* doing
  /// the work, so a state change made while the work runs queues a fresh one.
  pub fn release(&self) {
    self.0.store(false, Ordering::Release);
  }

  #[cfg(test)]
  pub fn is_set(&self) -> bool {
    self.0.load(Ordering::Acquire)
  }
}

/// Flags for every event kind that is coalesced. Shared between the UI side
/// (via `AppState::pending_io`) and the network task.
#[derive(Debug, Default)]
pub struct PendingIo {
  pub playback: PendingFlag,
  pub playlist_cover: PendingFlag,
}

impl PendingIo {
  /// The flag guarding `event`, if that kind of event is coalesced.
  pub fn flag_for(&self, event: &IoEvent) -> Option<&PendingFlag> {
    match event {
      IoEvent::GetCurrentPlayback => Some(&self.playback),
      IoEvent::RefreshPlaylistCover => Some(&self.playlist_cover),
      _ => None,
    }
  }

  /// Queue `event` unless an identical one is already queued. Never waits.
  ///
  /// A full channel drops the event and releases the flag so the next attempt
  /// (the next poll tick, the next cursor move) tries again. Returns whether
  /// the event was queued.
  pub fn send(&self, io_tx: &mpsc::Sender<IoEvent>, event: IoEvent) -> bool {
    let flag = self.flag_for(&event);
    if let Some(flag) = flag {
      if !flag.claim() {
        return false;
      }
    }
    match io_tx.try_send(event) {
      Ok(()) => true,
      Err(TrySendError::Full(_) | TrySendError::Closed(_)) => {
        if let Some(flag) = flag {
          flag.release();
        }
        false
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_one_poll_is_queued_until_dequeued() {
    let pending = PendingIo::default();
    let (tx, mut rx) = mpsc::channel(8);
    assert!(pending.send(&tx, IoEvent::GetCurrentPlayback));
    assert!(!pending.send(&tx, IoEvent::GetCurrentPlayback));
    assert!(!pending.send(&tx, IoEvent::GetCurrentPlayback));
    assert!(matches!(rx.try_recv(), Ok(IoEvent::GetCurrentPlayback)));
    assert!(rx.try_recv().is_err(), "duplicates must not be queued");

    // The network task releases on dequeue; the next tick queues again.
    pending.playback.release();
    assert!(pending.send(&tx, IoEvent::GetCurrentPlayback));
    assert!(matches!(rx.try_recv(), Ok(IoEvent::GetCurrentPlayback)));
  }

  #[test]
  fn kinds_are_deduped_independently() {
    let pending = PendingIo::default();
    let (tx, mut rx) = mpsc::channel(8);
    assert!(pending.send(&tx, IoEvent::GetCurrentPlayback));
    assert!(pending.send(&tx, IoEvent::RefreshPlaylistCover));
    assert!(!pending.send(&tx, IoEvent::RefreshPlaylistCover));
    assert!(matches!(rx.try_recv(), Ok(IoEvent::GetCurrentPlayback)));
    assert!(matches!(rx.try_recv(), Ok(IoEvent::RefreshPlaylistCover)));
    assert!(rx.try_recv().is_err());
  }

  #[test]
  fn full_channel_does_not_block_and_releases_the_flag() {
    let pending = PendingIo::default();
    let (tx, mut rx) = mpsc::channel(1);
    tx.try_send(IoEvent::NextTrack).unwrap();
    // Returns immediately instead of waiting for capacity.
    assert!(!pending.send(&tx, IoEvent::GetCurrentPlayback));
    assert!(
      !pending.playback.is_set(),
      "a dropped poll must not leave the flag stuck, or polling stops for good"
    );
    let _ = rx.try_recv();
    assert!(pending.send(&tx, IoEvent::GetCurrentPlayback));
  }

  #[test]
  fn uncoalesced_events_are_always_sent() {
    let pending = PendingIo::default();
    let (tx, mut rx) = mpsc::channel(8);
    assert!(pending.send(&tx, IoEvent::NextTrack));
    assert!(pending.send(&tx, IoEvent::NextTrack));
    assert!(rx.try_recv().is_ok());
    assert!(rx.try_recv().is_ok());
  }
}
