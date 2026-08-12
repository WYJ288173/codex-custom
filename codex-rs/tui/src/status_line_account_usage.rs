use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::Utc;
#[cfg(test)]
use codex_app_server_protocol::GetAccountTokenUsageResponse;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration as StdDuration;
use std::time::Instant;

pub(crate) const REFRESH_INTERVAL: StdDuration = StdDuration::from_secs(10 * 60);
const ACCOUNT_USAGE_TIMEZONE_OFFSET_HOURS: i64 = 8;

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
        result: Result<AccountUsageSummary, String>,
    ) -> bool {
        if self.pending_request_id != Some(request_id) {
            return false;
        }
        self.pending_request_id = None;
        if let Ok(summary) = result {
            self.summary = Some(summary);
        }
        true
    }

    pub(crate) fn summary(&self) -> Option<&AccountUsageSummary> {
        self.summary.as_ref()
    }
}

#[cfg(test)]
fn summarize_account_usage(
    response: &GetAccountTokenUsageResponse,
    today: NaiveDate,
) -> AccountUsageSummary {
    let Some(buckets) = response.daily_usage_buckets.as_ref() else {
        return AccountUsageSummary {
            total: response.summary.lifetime_tokens.map(|value| value.max(0)),
            ..Default::default()
        };
    };

    let parsed = buckets
        .iter()
        .filter_map(|bucket| {
            parse_bucket_start_date(&bucket.start_date).map(|date| (date, bucket.tokens.max(0)))
        })
        .collect::<Vec<_>>();
    let effective_today = effective_usage_date(&parsed, today);
    let week_start = effective_today
        - Duration::days(i64::from(effective_today.weekday().num_days_from_monday()));
    let month_start = effective_today - Duration::days(i64::from(effective_today.day0()));

    let mut daily = 0;
    let mut weekly = 0;
    let mut monthly = 0;
    for (date, tokens) in parsed {
        if date == effective_today {
            daily += tokens;
        }
        if (week_start..=effective_today).contains(&date) {
            weekly += tokens;
        }
        if (month_start..=effective_today).contains(&date) {
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
fn effective_usage_date(buckets: &[(NaiveDate, i64)], today: NaiveDate) -> NaiveDate {
    if buckets.iter().any(|(date, _)| *date == today) {
        return today;
    }
    buckets
        .iter()
        .filter_map(|(date, _)| (*date <= today).then_some(*date))
        .max()
        .unwrap_or(today)
}

#[cfg(test)]
fn parse_bucket_start_date(value: &str) -> Option<NaiveDate> {
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(
            (datetime.naive_utc() + Duration::hours(ACCOUNT_USAGE_TIMEZONE_OFFSET_HOURS)).date(),
        );
    }

    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|datetime| (datetime + Duration::hours(ACCOUNT_USAGE_TIMEZONE_OFFSET_HOURS)).date())
        .or_else(|| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .or_else(|| {
            value
                .get(..10)
                .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        })
}

fn account_usage_today_east_8() -> NaiveDate {
    (Utc::now().naive_utc() + Duration::hours(ACCOUNT_USAGE_TIMEZONE_OFFSET_HOURS)).date()
}

pub(crate) fn summarize_local_rollout_usage(codex_home: &Path) -> io::Result<AccountUsageSummary> {
    summarize_local_rollout_usage_for_day(codex_home, account_usage_today_east_8())
}

fn summarize_local_rollout_usage_for_day(
    codex_home: &Path,
    today: NaiveDate,
) -> io::Result<AccountUsageSummary> {
    let sessions_dir = codex_home.join("sessions");
    let mut events = Vec::new();
    collect_rollout_usage_events(&sessions_dir, &mut events)?;
    Ok(summarize_usage_events(events, today))
}

fn collect_rollout_usage_events(path: &Path, events: &mut Vec<(NaiveDate, i64)>) -> io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.is_file() {
        if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            let contents = fs::read_to_string(path)?;
            extend_rollout_token_usage_events(contents.lines(), events);
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        collect_rollout_usage_events(&entry.path(), events)?;
    }
    Ok(())
}

#[cfg(test)]
fn summarize_rollout_token_usage_lines<I, S>(lines: I, today: NaiveDate) -> AccountUsageSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut events = Vec::new();
    extend_rollout_token_usage_events(lines, &mut events);
    summarize_usage_events(events, today)
}

fn extend_rollout_token_usage_events<I, S>(lines: I, events: &mut Vec<(NaiveDate, i64)>)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut previous_total: Option<i64> = None;
    for line in lines {
        let Some((date, total_tokens)) = parse_rollout_token_count(line.as_ref()) else {
            continue;
        };
        let delta = previous_total
            .map(|previous| {
                if total_tokens >= previous {
                    total_tokens - previous
                } else {
                    total_tokens
                }
            })
            .unwrap_or(total_tokens)
            .max(0);
        previous_total = Some(total_tokens);
        if delta > 0 {
            events.push((date, delta));
        }
    }
}

fn parse_rollout_token_count(line: &str) -> Option<(NaiveDate, i64)> {
    let value: Value = serde_json::from_str(line).ok()?;
    let payload = value.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }
    let timestamp = value.get("timestamp")?.as_str()?;
    let date = parse_event_date_east_8(timestamp)?;
    let total_tokens = payload
        .get("info")?
        .get("total_token_usage")?
        .get("total_tokens")?
        .as_i64()?;
    Some((date, total_tokens))
}

fn parse_event_date_east_8(value: &str) -> Option<NaiveDate> {
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(
            (datetime.naive_utc() + Duration::hours(ACCOUNT_USAGE_TIMEZONE_OFFSET_HOURS)).date(),
        );
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|datetime| (datetime + Duration::hours(ACCOUNT_USAGE_TIMEZONE_OFFSET_HOURS)).date())
}

fn summarize_usage_events<I>(events: I, today: NaiveDate) -> AccountUsageSummary
where
    I: IntoIterator<Item = (NaiveDate, i64)>,
{
    let week_start = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let month_start = today - Duration::days(i64::from(today.day0()));
    let mut daily = 0;
    let mut weekly = 0;
    let mut monthly = 0;
    let mut total = 0;

    for (date, tokens) in events {
        let tokens = tokens.max(0);
        total += tokens;
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
        total: Some(total),
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
    fn summarizes_iso_datetime_bucket_dates() {
        let response = GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: Some(42_100_000),
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_usage_buckets: Some(vec![AccountTokenUsageDailyBucket {
                start_date: "2026-06-29T00:00:00Z".into(),
                tokens: 500,
            }]),
        };

        let result = summarize_account_usage(
            &response,
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        );

        assert_eq!(result.daily, Some(500));
        assert_eq!(result.weekly, Some(500));
        assert_eq!(result.monthly, Some(500));
    }

    #[test]
    fn converts_rfc3339_bucket_dates_to_east_8_before_summarizing() {
        let response = GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: Some(42_100_000),
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_usage_buckets: Some(vec![AccountTokenUsageDailyBucket {
                start_date: "2026-06-28T17:30:00Z".into(),
                tokens: 500,
            }]),
        };

        let result = summarize_account_usage(
            &response,
            NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
        );

        assert_eq!(result.daily, Some(500));
        assert_eq!(result.weekly, Some(500));
        assert_eq!(result.monthly, Some(500));
    }

    #[test]
    fn summarizes_latest_backend_bucket_when_local_today_is_ahead() {
        let response = GetAccountTokenUsageResponse {
            summary: AccountTokenUsageSummary {
                lifetime_tokens: Some(174_144_367),
                peak_daily_tokens: Some(62_805_203),
                longest_running_turn_sec: Some(10_002),
                current_streak_days: Some(4),
                longest_streak_days: Some(4),
            },
            daily_usage_buckets: Some(vec![
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-28".into(),
                    tokens: 6_660_825,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-29".into(),
                    tokens: 62_805_203,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-06-30".into(),
                    tokens: 60_565_847,
                },
                AccountTokenUsageDailyBucket {
                    start_date: "2026-07-01".into(),
                    tokens: 44_112_492,
                },
            ]),
        };

        let result = summarize_account_usage(
            &response,
            NaiveDate::from_ymd_opt(2026, 7, 2).expect("valid date"),
        );

        assert_eq!(
            result,
            AccountUsageSummary {
                daily: Some(44_112_492),
                weekly: Some(167_483_542),
                monthly: Some(44_112_492),
                total: Some(174_144_367),
            }
        );
    }

    #[test]
    fn local_rollout_usage_buckets_by_east_8_day_boundary() {
        let lines = [
            r#"{"timestamp":"2026-07-01T15:30:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":1000},"last_token_usage":{"input_tokens":1000,"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":1000},"model_context_window":353400},"rate_limits":null}}"#,
            r#"{"timestamp":"2026-07-01T16:30:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":4500,"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":4500},"last_token_usage":{"input_tokens":3500,"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":3500},"model_context_window":353400},"rate_limits":null}}"#,
        ];

        let summary = summarize_rollout_token_usage_lines(
            lines,
            NaiveDate::from_ymd_opt(2026, 7, 2).expect("valid date"),
        );

        assert_eq!(
            summary,
            AccountUsageSummary {
                daily: Some(3_500),
                weekly: Some(4_500),
                monthly: Some(4_500),
                total: Some(4_500),
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
    fn tracker_requests_immediately_then_waits_ten_minutes() {
        let now = Instant::now();
        let mut tracker = AccountUsageTracker::default();
        assert_eq!(tracker.request_if_due(now), Some(0));
        assert_eq!(
            tracker.request_if_due(now + StdDuration::from_secs(599)),
            None
        );
        assert!(tracker.complete(
            0,
            Ok(summarize_account_usage(
                &response(),
                NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
            ))
        ));
        assert_eq!(
            tracker.request_if_due(now + StdDuration::from_secs(599)),
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
            Ok(summarize_account_usage(
                &response(),
                NaiveDate::from_ymd_opt(2026, 6, 29).expect("valid date"),
            ))
        ));
        assert_eq!(
            tracker.summary().and_then(|summary| summary.total),
            Some(42_100_000)
        );

        let second = tracker
            .request_if_due(now + REFRESH_INTERVAL)
            .expect("refresh request");
        assert!(tracker.complete(second, Err("offline".to_string())));
        assert_eq!(
            tracker.summary().and_then(|summary| summary.total),
            Some(42_100_000)
        );
    }
}
