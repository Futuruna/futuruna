//! Bounded CLI patience for a transient host-provider failure, not admission.
//! The next warm slice must obtain a fresh resource permit in the unchanged
//! governor. Missing telemetry never authorizes work or reuses an old sample.

use std::time::Duration;

pub(crate) const MAX_TELEMETRY_RETRIES: u8 = 3;
pub(crate) const MAX_RECOVERY_RESERVE_WAITS: u8 = 6;

#[derive(Default)]
pub(crate) struct TelemetryRetryBudget {
    retries_without_progress: u8,
    reserve_waits_without_progress: u8,
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
            *self = Self::default();
        }
        if !supervised {
            return None;
        }
        let reserve_wait = match resource_pause_code {
            Some("telemetry_provider_unavailable")
                if self.retries_without_progress < MAX_TELEMETRY_RETRIES =>
            {
                false
            }
            Some("resource_reserve_backoff")
                if self.retries_without_progress > 0
                    && self.reserve_waits_without_progress < MAX_RECOVERY_RESERVE_WAITS =>
            {
                true
            }
            _ => return None,
        };
        // Reserve patience exists only inside an already-started telemetry
        // recovery episode. Five seconds matches the contained host sampling
        // cadence; each subsequent slice still needs entirely fresh admission.
        let delay = if reserve_wait {
            Duration::from_secs(5)
        } else {
            Duration::from_secs(1_u64 << self.retries_without_progress)
        };
        // Leave time for a fresh call. Never extend the original invocation
        // deadline, including when a cooldown consumes the remaining budget.
        if remaining_runtime.is_some_and(|remaining| remaining <= delay) {
            return None;
        }
        if reserve_wait {
            self.reserve_waits_without_progress += 1;
        } else {
            self.retries_without_progress += 1;
        }
        Some(delay)
    }

    pub(crate) fn attempts(&self) -> u8 {
        self.retries_without_progress
    }

    pub(crate) fn reserve_waits(&self) -> u8 {
        self.reserve_waits_without_progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVIDER_FAILURE: Option<&str> = Some("telemetry_provider_unavailable");
    const RESERVE_BACKOFF: Option<&str> = Some("resource_reserve_backoff");

    #[test]
    fn telemetry_recovery_has_bounded_reserve_patience_without_resetting_either_budget() {
        let mut budget = TelemetryRetryBudget::default();
        assert!(budget
            .next_delay(true, PROVIDER_FAILURE, false, None)
            .is_some());
        for attempt in 1..=MAX_RECOVERY_RESERVE_WAITS {
            assert_eq!(
                budget.next_delay(true, RESERVE_BACKOFF, false, None),
                Some(Duration::from_secs(5))
            );
            assert_eq!(budget.reserve_waits(), attempt);
            assert_eq!(budget.next_delay(true, None, false, None), None);
        }
        assert_eq!(budget.next_delay(true, RESERVE_BACKOFF, false, None), None);
        assert_eq!(budget.attempts(), 1);
        for seconds in [2, 4] {
            assert_eq!(
                budget.next_delay(true, PROVIDER_FAILURE, false, None),
                Some(Duration::from_secs(seconds))
            );
            assert_eq!(budget.next_delay(true, RESERVE_BACKOFF, false, None), None);
        }
        assert_eq!(budget.next_delay(true, PROVIDER_FAILURE, false, None), None);
        // Useful work ends the episode; a reserve pause alone cannot start one.
        assert_eq!(budget.next_delay(true, RESERVE_BACKOFF, true, None), None);
        assert_eq!(budget.reserve_waits(), 0);
        assert!(budget
            .next_delay(true, PROVIDER_FAILURE, false, None)
            .is_some());
        assert!(budget
            .next_delay(true, RESERVE_BACKOFF, false, None)
            .is_some());
    }

    #[test]
    fn reserve_recovery_keeps_deadline_supervision_and_other_failure_boundaries() {
        let mut budget = TelemetryRetryBudget::default();
        assert!(budget
            .next_delay(true, PROVIDER_FAILURE, false, None)
            .is_some());
        assert_eq!(budget.next_delay(false, RESERVE_BACKOFF, false, None), None);
        assert_eq!(
            budget.next_delay(true, RESERVE_BACKOFF, false, Some(Duration::from_secs(5))),
            None
        );
        for code in [
            "resource_memory_pressure_critical",
            "resource_oom_risk",
            "resource_swap_growth",
            "telemetry_incoherent_host_facts",
            "resource_governor_failed",
            "mechanism_replay_failed",
        ] {
            assert_eq!(budget.next_delay(true, Some(code), false, None), None);
        }
        assert_eq!(budget.reserve_waits(), 0);
        assert_eq!(
            budget.next_delay(
                true,
                RESERVE_BACKOFF,
                false,
                Some(Duration::from_millis(5001))
            ),
            Some(Duration::from_secs(5))
        );
    }

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
