use chrono::Datelike;
use chrono::Duration;
use chrono::NaiveDate;
use codex_app_server_protocol::GetAccountTokenUsageResponse;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AccountUsageSummary {
    pub(crate) daily: Option<i64>,
    pub(crate) weekly: Option<i64>,
    pub(crate) monthly: Option<i64>,
    pub(crate) total: Option<i64>,
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
}
