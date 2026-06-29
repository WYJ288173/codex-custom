use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use codex_app_server_protocol::GetAccountTokenUsageResponse;
use std::time::Duration as StdDuration;
use std::time::Instant;

pub(crate) const REFRESH_INTERVAL: StdDuration = StdDuration::from_secs(5 * 60);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AccountUsageSummary {
    pub(crate) daily: Option<i64>,
    pub(crate) weekly: Option<i64>,
    pub(crate) monthly: Option<i64>,
    pub(crate) total: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AccountUsageTracker {
    summary: Option<AccountUsageSummary>,
    pending_request_id: Option<u64>,
    last_requested_at: Option<Instant>,
    next_request_id: u64,
}

impl AccountUsageTracker {
    pub(crate) fn request_if_due(&mut self, now: Instant) -> Option<u64> {
        if self.pending_request_id.is_some()
            || self
                .last_requested_at
                .is_some_and(|last| now.saturating_duration_since(last) < REFRESH_INTERVAL)
        {
            return None;
        }

        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.pending_request_id = Some(request_id);
        self.last_requested_at = Some(now);
        Some(request_id)
    }

    pub(crate) fn complete(
        &mut self,
        request_id: u64,
        result: Result<GetAccountTokenUsageResponse, String>,
        today: NaiveDate,
    ) -> bool {
        if self.pending_request_id != Some(request_id) {
            return false;
        }
        self.pending_request_id = None;
        if let Ok(response) = result {
            self.summary = Some(summarize_account_usage(&response, today));
        }
        true
    }

    pub(crate) fn summary(&self) -> Option<&AccountUsageSummary> {
        self.summary.as_ref()
    }
}

pub(crate) fn summarize_account_usage(
    response: &GetAccountTokenUsageResponse,
    today: NaiveDate,
) -> AccountUsageSummary {
    let Some(buckets) = response.daily_usage_buckets.as_ref() else {
        return AccountUsageSummary {
            total: response.summary.lifetime_tokens.map(|value| value.max(0)),
            ..Default::default()
        };
    };

    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let month_start = today.with_day(1).expect("day one exists");
    let parsed = buckets.iter().filter_map(|bucket| {
        NaiveDate::parse_from_str(&bucket.start_date, "%Y-%m-%d")
            .ok()
            .map(|date| (date, bucket.tokens.max(0)))
    });

    let mut daily = 0;
    let mut weekly = 0;
    let mut monthly = 0;
    for (date, tokens) in parsed {
        if date == today {
            daily += tokens;
        }
        if (week_start..=today).contains(&date) {
            weekly += tokens;
        }
        if (month_start..=today).contains(&date) {
            monthly += tokens;
        }
    }

    AccountUsageSummary {
        daily: Some(daily),
        weekly: Some(weekly),
        monthly: Some(monthly),
        total: response.summary.lifetime_tokens.map(|value| value.max(0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AccountTokenUsageDailyBucket;
    use codex_app_server_protocol::AccountTokenUsageSummary;

    fn response() -> GetAccountTokenUsageResponse {
        GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: Some(42_100_000),
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_usage_buckets: Some(vec![
                AccountTokenUsageDailyBucket {
                    start_date: "2026-05-31".into(),
                    tokens: 100,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-01".into(),
                    tokens: 200,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-22".into(),
                    tokens: 300,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-28".into(),
                    tokens: 400,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-29".into(),
                    tokens: 500,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "invalid".into(),
                    tokens: 99_999,
                },
            ]),
        }
    }

    #[test]
    fn summarizes_today_monday_week_month_and_lifetime() {
        let result = summarize_account_usage(
            &response(),
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        );
        assert_eq!(
            result,
            AccountUsageSummary {
                daily: Some(500),
                weekly: Some(500),
                monthly: Some(1_400),
                total: Some(42_100_000),
            }
        );
    }

    #[test]
    fn missing_buckets_preserves_lifetime_only() {
        let mut response = response();
        response.daily_usage_buckets = None;
        let result = summarize_account_usage(
            &response,
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        );
        assert_eq!(result.daily, None);
        assert_eq!(result.weekly, None);
        assert_eq!(result.monthly, None);
        assert_eq!(result.total, Some(42_100_000));
    }

    #[test]
    fn tracker_requests_immediately_then_waits_five_minutes() {
        let now = Instant::now();
        let mut tracker = AccountUsageTracker::default();
        assert_eq!(tracker.request_if_due(now), Some(0));
        assert_eq!(
            tracker.request_if_due(now + StdDuration::from_secs(299)),
            None
        );
        assert!(tracker.complete(
            0,
            Ok(response()),
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        ));
        assert_eq!(
            tracker.request_if_due(now + StdDuration::from_secs(299)),
            None
        );
        assert_eq!(tracker.request_if_due(now + REFRESH_INTERVAL), Some(1));
    }

    #[test]
    fn tracker_retains_last_success_after_refresh_error() {
        let now = Instant::now();
        let mut tracker = AccountUsageTracker::default();
        let first = tracker.request_if_due(now).expect("initial request");
        assert!(tracker.complete(
            first,
            Ok(response()),
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        ));
        assert_eq!(
            tracker.summary().and_then(|summary| summary.total),
            Some(42_100_000)
        );

        let second = tracker
            .request_if_due(now + REFRESH_INTERVAL)
            .expect("refresh request");
        assert!(tracker.complete(
            second,
            Err("offline".to_string()),
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        ));
        assert_eq!(
            tracker.summary().and_then(|summary| summary.total),
            Some(42_100_000)
        );
    }
}
