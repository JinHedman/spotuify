//! Poll backoff and rate-limit handling.
//!
//! Without this every failed poll fires again after `poll_interval_ms`, which
//! keeps a rate limit in place and floods the status line during an outage.
//! Kept free of the network so the timing rules are unit-testable.

use crate::app::AppState;
use rspotify::http::HttpError;
use rspotify::ClientError;
use std::time::{Duration, Instant};

/// First retry delay after a failed poll; doubles per consecutive failure.
const BASE_DELAY: Duration = Duration::from_secs(2);
/// Upper bound on the exponential delay.
const MAX_DELAY: Duration = Duration::from_secs(60);

/// What kind of failure a request hit, as far as retrying is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
  /// HTTP 429. `retry_after` is Spotify's `Retry-After`, when present and
  /// parseable.
  RateLimited { retry_after: Option<Duration> },
  /// The request never got a response: no network, DNS, TLS, timeout.
  Unreachable,
  /// Anything else — a non-429 status, a parse error.
  Other,
}

impl Failure {
  pub fn classify(err: &anyhow::Error) -> Self {
    for cause in err.chain() {
      let Some(ClientError::Http(http)) = cause.downcast_ref::<ClientError>() else {
        continue;
      };
      return match http.as_ref() {
        HttpError::StatusCode(resp) if resp.status().as_u16() == 429 => Failure::RateLimited {
          retry_after: resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after),
        },
        HttpError::StatusCode(_) => Failure::Other,
        HttpError::Client(_) => Failure::Unreachable,
      };
    }
    Failure::Other
  }
}

/// Longest `Retry-After` honoured. The header is untrusted input: an absurd
/// value would overflow `Instant + Duration` and panic the network task.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60 * 60);

/// `Retry-After` as delta-seconds, clamped to `MAX_RETRY_AFTER`. Spotify sends
/// integers; the HTTP-date form is not handled and falls back to the
/// exponential delay.
pub fn parse_retry_after(value: &str) -> Option<Duration> {
  value
    .trim()
    .parse::<u64>()
    .ok()
    .map(|secs| Duration::from_secs(secs).min(MAX_RETRY_AFTER))
}

/// Exponential delay for the `n`th consecutive failure (1-based), capped.
pub fn backoff_delay(consecutive_failures: u32) -> Duration {
  let shift = consecutive_failures.saturating_sub(1).min(16);
  BASE_DELAY.saturating_mul(1 << shift).min(MAX_DELAY)
}

#[derive(Debug, Default)]
pub struct PollBackoff {
  consecutive_failures: u32,
  /// Polls are skipped until this instant.
  resume_at: Option<Instant>,
  /// Last notice posted and when, so an unchanged message is not re-posted
  /// while it is still on screen.
  last_notice: Option<(String, Instant)>,
}

impl PollBackoff {
  pub fn allows_poll(&self, now: Instant) -> bool {
    self.resume_at.is_none_or(|t| now >= t)
  }

  pub fn on_success(&mut self) {
    *self = Self::default();
  }

  /// Record a failed poll and suspend polling. Returns the notice to show, or
  /// `None` if the same text is still visible.
  ///
  /// `detail` is only used for `Failure::Other`, where there is no plain
  /// description to fall back on.
  pub fn on_failure(&mut self, failure: &Failure, detail: &str, now: Instant) -> Option<String> {
    let secs = self.suspend(failure, now);
    let text = match failure {
      Failure::RateLimited { .. } => format!("Rate limited, retrying in {secs}s"),
      Failure::Unreachable => format!("Can't reach Spotify, retrying in {secs}s"),
      Failure::Other => format!("Playback update failed ({detail}), retrying in {secs}s"),
    };
    self.post(text, now)
  }

  /// Record a 429 on a user action (play, save, next…). The limit applies to
  /// the whole app, so polling is suspended too. The action itself is not
  /// retried, so the notice names it and says it failed — "retrying" would
  /// promise something that never happens. Always returned: each failed
  /// action is news to the user.
  pub fn on_action_rate_limited(
    &mut self,
    action: &str,
    failure: &Failure,
    now: Instant,
  ) -> String {
    let secs = self.suspend(failure, now);
    format!("{action} failed: rate limited, try again in {secs}s")
  }

  /// Push `resume_at` out for `failure` and return the remaining wait in
  /// whole seconds, rounded up.
  fn suspend(&mut self, failure: &Failure, now: Instant) -> u64 {
    self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    let delay = match failure {
      // Honour Spotify's number as given; polling before it only extends the
      // limit. Floor at 1s so "0" cannot turn into a tight retry loop.
      Failure::RateLimited {
        retry_after: Some(d),
      } => (*d).max(Duration::from_secs(1)),
      _ => backoff_delay(self.consecutive_failures),
    };
    let until = now + delay;
    // Never shorten an existing suspension, e.g. a Retry-After from a user
    // action followed by a quicker ordinary failure.
    self.resume_at = Some(self.resume_at.map_or(until, |t| t.max(until)));

    let secs = self.resume_at.unwrap().saturating_duration_since(now);
    secs.as_secs() + u64::from(secs.subsec_nanos() > 0)
  }

  fn post(&mut self, text: String, now: Instant) -> Option<String> {
    if let Some((last, at)) = &self.last_notice {
      if *last == text && now.duration_since(*at) < AppState::NOTICE_TTL {
        return None;
      }
    }
    self.last_notice = Some((text.clone(), now));
    Some(text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn retry_after_parses_delta_seconds() {
    assert_eq!(parse_retry_after("12"), Some(Duration::from_secs(12)));
    assert_eq!(parse_retry_after(" 3 "), Some(Duration::from_secs(3)));
    assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), None);
    assert_eq!(parse_retry_after("-1"), None);
  }

  #[test]
  fn retry_after_is_clamped_so_resume_time_cannot_overflow() {
    assert_eq!(parse_retry_after("3600"), Some(MAX_RETRY_AFTER));
    assert_eq!(parse_retry_after("3601"), Some(MAX_RETRY_AFTER));
    assert_eq!(
      parse_retry_after(&u64::MAX.to_string()),
      Some(MAX_RETRY_AFTER)
    );
    // The value classify would produce must not panic when added to now.
    let mut b = PollBackoff::default();
    let limited = Failure::RateLimited {
      retry_after: parse_retry_after(&u64::MAX.to_string()),
    };
    assert!(b.on_failure(&limited, "", Instant::now()).is_some());
  }

  #[test]
  fn rate_limited_action_names_the_action_and_says_it_failed() {
    let now = Instant::now();
    let mut b = PollBackoff::default();
    let limited = Failure::RateLimited {
      retry_after: Some(Duration::from_secs(12)),
    };
    assert_eq!(
      b.on_action_rate_limited("Next track", &limited, now),
      "Next track failed: rate limited, try again in 12s"
    );
    // Polling is suspended as well; the limit is app-wide.
    assert!(!b.allows_poll(now + Duration::from_secs(11)));
    assert!(b.allows_poll(now + Duration::from_secs(12)));
    // A second failed action is reported again, not deduped.
    assert_eq!(
      b.on_action_rate_limited("Next track", &limited, now),
      "Next track failed: rate limited, try again in 12s"
    );
  }

  #[test]
  fn delay_doubles_and_caps() {
    let secs: Vec<u64> = (1..=8).map(|n| backoff_delay(n).as_secs()).collect();
    assert_eq!(secs, [2, 4, 8, 16, 32, 60, 60, 60]);
    assert_eq!(backoff_delay(u32::MAX), MAX_DELAY);
  }

  #[test]
  fn retry_after_suppresses_polls_until_it_elapses() {
    let now = Instant::now();
    let mut b = PollBackoff::default();
    assert!(b.allows_poll(now));
    let limited = Failure::RateLimited {
      retry_after: Some(Duration::from_secs(12)),
    };
    let notice = b.on_failure(&limited, "", now);
    assert_eq!(notice.as_deref(), Some("Rate limited, retrying in 12s"));
    assert!(!b.allows_poll(now + Duration::from_secs(11)));
    assert!(b.allows_poll(now + Duration::from_secs(12)));
  }

  #[test]
  fn zero_retry_after_still_waits() {
    let now = Instant::now();
    let mut b = PollBackoff::default();
    let limited = Failure::RateLimited {
      retry_after: Some(Duration::ZERO),
    };
    b.on_failure(&limited, "", now);
    assert!(!b.allows_poll(now));
  }

  #[test]
  fn rate_limit_without_header_uses_backoff() {
    let now = Instant::now();
    let mut b = PollBackoff::default();
    let limited = Failure::RateLimited { retry_after: None };
    assert_eq!(
      b.on_failure(&limited, "", now).as_deref(),
      Some("Rate limited, retrying in 2s")
    );
  }

  #[test]
  fn consecutive_failures_back_off_and_success_resets() {
    let mut now = Instant::now();
    let mut b = PollBackoff::default();
    let mut notices = Vec::new();
    for _ in 0..4 {
      notices.push(b.on_failure(&Failure::Unreachable, "", now).unwrap());
      now += backoff_delay(b.consecutive_failures);
      assert!(b.allows_poll(now));
    }
    assert_eq!(
      notices,
      [
        "Can't reach Spotify, retrying in 2s",
        "Can't reach Spotify, retrying in 4s",
        "Can't reach Spotify, retrying in 8s",
        "Can't reach Spotify, retrying in 16s",
      ]
    );

    b.on_success();
    assert!(b.allows_poll(now));
    assert_eq!(
      b.on_failure(&Failure::Unreachable, "", now).as_deref(),
      Some("Can't reach Spotify, retrying in 2s"),
      "success resets the exponent"
    );
  }

  #[test]
  fn a_shorter_failure_does_not_cut_a_retry_after_short() {
    let now = Instant::now();
    let mut b = PollBackoff::default();
    let limited = Failure::RateLimited {
      retry_after: Some(Duration::from_secs(30)),
    };
    b.on_failure(&limited, "", now);
    b.on_failure(&Failure::Unreachable, "", now);
    assert!(!b.allows_poll(now + Duration::from_secs(29)));
  }

  #[test]
  fn identical_notice_is_not_reposted_while_visible() {
    let now = Instant::now();
    let mut b = PollBackoff::default();
    let limited = Failure::RateLimited {
      retry_after: Some(Duration::from_secs(5)),
    };
    assert!(b.on_failure(&limited, "", now).is_some());
    assert!(
      b.on_failure(&limited, "", now).is_none(),
      "same text, still visible"
    );
    let later = now + AppState::NOTICE_TTL;
    assert!(
      b.on_failure(&limited, "", later).is_some(),
      "re-posted once the old one has expired"
    );
  }

  fn status_error(status: u16, retry_after: Option<&str>) -> anyhow::Error {
    let mut builder = http::Response::builder().status(status);
    if let Some(v) = retry_after {
      builder = builder.header("Retry-After", v);
    }
    let resp = builder.body("").unwrap();
    let err = ClientError::from(HttpError::StatusCode(resp.into()));
    // Wrapped in context the way handlers do, to prove the chain is walked.
    anyhow::Error::new(err).context("current_playback")
  }

  #[test]
  fn classifies_429_with_retry_after() {
    assert_eq!(
      Failure::classify(&status_error(429, Some("7"))),
      Failure::RateLimited {
        retry_after: Some(Duration::from_secs(7))
      }
    );
    assert_eq!(
      Failure::classify(&status_error(429, None)),
      Failure::RateLimited { retry_after: None }
    );
    assert_eq!(Failure::classify(&status_error(500, None)), Failure::Other);
  }

  #[test]
  fn plain_errors_are_not_rate_limits() {
    let err = anyhow::anyhow!("boom");
    assert_eq!(Failure::classify(&err), Failure::Other);
  }
}
