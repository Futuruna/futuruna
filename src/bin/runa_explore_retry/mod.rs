//! Bounded CLI patience for a transient host-provider failure, not admission.
//! The next warm slice must obtain a fresh resource permit in the unchanged
//! governor. Missing telemetry never authorizes work or reuses an old sample.

use std::time::Duration;

pub(crate) const MAX_TELEMETRY_RETRIES: u8 = 3;

#[derive(Default)]
pub(crate) struct TelemetryRetryBudget {
    retries_without_progress: u8,
}

impl TelemetryRetryBudget {
    pub(crate) fn next_delay(
        &mut self,
        supervised: bool,
        resource_pause_code: Option<&str>,
        made_progress: bool,
        remaining_runtime: Option<Duration>,
    ) -> Option<Duration> {
        if made_progress {
            self.retries_without_progress = 0;
        }
        if !supervised
            || resource_pause_code != Some("telemetry_provider_unavailable")
            || self.retries_without_progress >= MAX_TELEMETRY_RETRIES
        {
            return None;
        }
        let delay = Duration::from_secs(1_u64 << self.retries_without_progress);
        // Leave time for a fresh call. Never extend the original invocation
        // deadline, including when a cooldown consumes the remaining budget.
        if remaining_runtime.is_some_and(|remaining| remaining <= delay) {
            return None;
        }
        self.retries_without_progress += 1;
        Some(delay)
    }

    pub(crate) fn attempts(&self) -> u8 {
        self.retries_without_progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVIDER_FAILURE: Option<&str> = Some("telemetry_provider_unavailable");

    #[test]
    fn supervised_provider_failure_has_three_bounded_warm_retries() {
        let mut budget = TelemetryRetryBudget::default();
        for seconds in [1, 2, 4] {
            assert_eq!(
                budget.next_delay(true, PROVIDER_FAILURE, false, None),
                Some(Duration::from_secs(seconds))
            );
        }
        assert_eq!(budget.attempts(), MAX_TELEMETRY_RETRIES);
        for _ in 0..2 {
            assert_eq!(budget.next_delay(true, PROVIDER_FAILURE, false, None), None);
        }
    }

    #[test]
    fn unsupervised_and_other_resource_or_semantic_pauses_are_not_retried() {
        let mut budget = TelemetryRetryBudget::default();
        assert_eq!(
            budget.next_delay(false, PROVIDER_FAILURE, false, None),
            None
        );
        for code in [
            None,
            Some("resource_reserve_backoff"),
            Some("telemetry_sample_expired"),
            Some("telemetry_cached_admission_expired"),
            Some("telemetry_incoherent_host_facts"),
            Some("resource_governor_failed"),
            Some("mechanism_replay_failed"),
        ] {
            assert_eq!(budget.next_delay(true, code, false, None), None);
        }
        assert_eq!(budget.attempts(), 0);
    }

    #[test]
    fn cooldown_must_fit_strictly_inside_the_original_deadline() {
        let mut budget = TelemetryRetryBudget::default();
        for remaining in [
            Duration::ZERO,
            Duration::from_millis(999),
            Duration::from_secs(1),
        ] {
            assert_eq!(
                budget.next_delay(true, PROVIDER_FAILURE, false, Some(remaining)),
                None
            );
            assert_eq!(budget.attempts(), 0);
        }
        assert_eq!(
            budget.next_delay(
                true,
                PROVIDER_FAILURE,
                false,
                Some(Duration::from_secs(1) + Duration::from_nanos(1))
            ),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            budget.next_delay(true, PROVIDER_FAILURE, false, Some(Duration::from_secs(2))),
            None
        );
        assert_eq!(budget.attempts(), 1);
    }

    #[test]
    fn only_real_progress_replenishes_the_retry_budget() {
        let mut budget = TelemetryRetryBudget::default();
        for _ in 0..MAX_TELEMETRY_RETRIES {
            assert!(budget
                .next_delay(true, PROVIDER_FAILURE, false, None)
                .is_some());
        }
        // An intervening runtime/coordination pause without progress cannot
        // start an endless failure -> runtime pause -> failure cycle.
        assert_eq!(budget.next_delay(true, None, false, None), None);
        assert_eq!(budget.next_delay(true, PROVIDER_FAILURE, false, None), None);
        assert_eq!(budget.next_delay(true, None, true, None), None);
        assert_eq!(
            budget.next_delay(true, PROVIDER_FAILURE, false, None),
            Some(Duration::from_secs(1))
        );
        assert_eq!(budget.attempts(), 1);
        // Useful work followed by another failure begins a fresh episode.
        assert_eq!(
            budget.next_delay(true, PROVIDER_FAILURE, true, None),
            Some(Duration::from_secs(1))
        );
        assert_eq!(budget.attempts(), 1);
    }
}
