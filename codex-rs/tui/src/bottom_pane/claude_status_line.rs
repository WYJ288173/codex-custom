use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::StatusLineValue;
use crate::status_line_account_usage::AccountUsageSummary;

const MODEL: Color = Color::Green;
const CONTEXT: Color = Color::Magenta;
const PATH: Color = Color::Cyan;
const INPUT: Color = Color::Blue;
const OUTPUT: Color = Color::Yellow;
const TOTAL: Color = Color::Red;
const MUTED: Color = Color::DarkGray;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ClaudeStatusLineData {
    pub(crate) model: String,
    pub(crate) reasoning: Option<String>,
    pub(crate) current_dir: String,
    pub(crate) git_branch: Option<String>,
    pub(crate) context_used_percent: Option<i64>,
    pub(crate) session_input_tokens: i64,
    pub(crate) session_output_tokens: i64,
    pub(crate) account_usage: Option<AccountUsageSummary>,
    pub(crate) five_hour_limit: Option<String>,
    pub(crate) weekly_limit: Option<String>,
}

pub(crate) fn render_claude_status_line(
    data: &ClaudeStatusLineData,
    width: u16,
) -> StatusLineValue {
    let width = usize::from(width.max(1));
    let narrow = width < 140;
    let path = abbreviate_home_and_parents(
        &data.current_dir,
        if narrow {
            (width / 3).min(12)
        } else {
            width / 2
        },
    );

    let mut identity = model_and_reasoning_spans(data);
    push_separator(&mut identity);
    identity.push(Span::styled(path, Style::default().fg(PATH)));
    if let Some(branch) = data.git_branch.as_ref().filter(|branch| !branch.is_empty()) {
        push_separator(&mut identity);
        identity.push(Span::styled(branch.clone(), Style::default().fg(MODEL)));
    }

    let context = context_spans(data.context_used_percent);
    let session = session_spans(data.session_input_tokens, data.session_output_tokens);
    let account = account_spans(data.account_usage.as_ref(), narrow);
    let limits = limit_spans(
        data.five_hour_limit.as_deref(),
        data.weekly_limit.as_deref(),
        narrow,
    );
    let lines = if narrow {
        vec![
            Line::from(identity),
            Line::from(join_sections([context, session, limits])),
            Line::from(account),
        ]
    } else {
        vec![
            Line::from(identity),
            Line::from(join_sections([context, session, account, limits])),
        ]
    };
    lines
        .into_iter()
        .map(|line| truncate_styled_line(line, width))
        .collect()
}

fn model_and_reasoning_spans(data: &ClaudeStatusLineData) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        data.model.clone(),
        Style::default().fg(MODEL).add_modifier(Modifier::BOLD),
    )];
    if let Some(reasoning) = data.reasoning.as_deref().filter(|value| !value.is_empty()) {
        spans.push(Span::styled(
            format!(" · {reasoning}"),
            Style::default().fg(MODEL),
        ));
    }
    spans
}

fn context_spans(percent: Option<i64>) -> Vec<Span<'static>> {
    let label = match percent {
        Some(percent) => {
            let percent = percent.clamp(0, 100);
            format!("Ctx {percent}% {}", context_bar(percent))
        }
        None => "Ctx — ░░░░░░░░░░".to_string(),
    };
    vec![Span::styled(label, Style::default().fg(CONTEXT))]
}

fn session_spans(input: i64, output: i64) -> Vec<Span<'static>> {
    vec![
        Span::styled("Session ", Style::default().fg(MUTED)),
        Span::styled(
            format!("↑{}", compact_tokens(Some(input))),
            Style::default().fg(INPUT),
        ),
        Span::raw(" "),
        Span::styled(
            format!("↓{}", compact_tokens(Some(output))),
            Style::default().fg(OUTPUT),
        ),
    ]
}

fn account_spans(usage: Option<&AccountUsageSummary>, narrow: bool) -> Vec<Span<'static>> {
    let daily = usage.and_then(|value| value.daily);
    let weekly = usage.and_then(|value| value.weekly);
    let monthly = usage.and_then(|value| value.monthly);
    let total = usage.and_then(|value| value.total);
    let separator = if narrow { " │ " } else { " " };
    vec![
        Span::styled(
            format!("Today {}", compact_tokens(daily)),
            Style::default().fg(MODEL),
        ),
        Span::styled(separator, Style::default().fg(MUTED)),
        Span::styled(
            format!("Week {}", compact_tokens(weekly)),
            Style::default().fg(OUTPUT),
        ),
        Span::styled(separator, Style::default().fg(MUTED)),
        Span::styled(
            format!("Month {}", compact_tokens(monthly)),
            Style::default().fg(PATH),
        ),
        Span::styled(separator, Style::default().fg(MUTED)),
        Span::styled(
            format!("Total {}", compact_tokens(total)),
            Style::default().fg(TOTAL),
        ),
    ]
}

fn limit_spans(five_hour: Option<&str>, weekly: Option<&str>, narrow: bool) -> Vec<Span<'static>> {
    let five_hour = five_hour.unwrap_or("5h —");
    let weekly = weekly
        .map(|value| {
            if narrow {
                value.replacen("Limit/week", "Week limit", 1)
            } else {
                value.to_string()
            }
        })
        .unwrap_or_else(|| {
            if narrow {
                "Week limit —".to_string()
            } else {
                "Limit/week —".to_string()
            }
        });
    vec![
        Span::styled(five_hour.to_string(), limit_style(five_hour)),
        Span::styled(" · ", Style::default().fg(MUTED)),
        Span::styled(weekly.clone(), limit_style(&weekly)),
    ]
}

fn limit_style(value: &str) -> Style {
    let percent = value
        .split_whitespace()
        .find_map(|part| part.strip_suffix('%'))
        .and_then(|part| part.parse::<f64>().ok());
    Style::default().fg(if percent.is_some_and(|value| value >= 80.0) {
        TOTAL
    } else {
        MUTED
    })
}

fn join_sections<const N: usize>(sections: [Vec<Span<'static>>; N]) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    for section in sections {
        if !result.is_empty() {
            push_separator(&mut result);
        }
        result.extend(section);
    }
    result
}

fn push_separator(spans: &mut Vec<Span<'static>>) {
    spans.push(Span::styled(" │ ", Style::default().fg(MUTED)));
}

fn compact_tokens(value: Option<i64>) -> String {
    let Some(value) = value else {
        return "—".to_string();
    };
    let value = value.max(0) as f64;
    if value >= 1_000_000.0 {
        compact_scaled(value / 1_000_000.0, "M")
    } else if value >= 1_000.0 {
        compact_scaled(value / 1_000.0, "K")
    } else {
        format!("{value:.0}")
    }
}

fn compact_scaled(value: f64, suffix: &str) -> String {
    if value >= 100.0 || (value.fract() * 10.0).round() == 0.0 {
        format!("{value:.0}{suffix}")
    } else {
        format!("{value:.1}{suffix}")
    }
}

fn context_bar(percent: i64) -> String {
    let filled = (percent.clamp(0, 100) * 10) / 100;
    format!(
        "{}{}",
        "▓".repeat(filled as usize),
        "░".repeat((10 - filled) as usize)
    )
}

fn abbreviate_home_and_parents(path: &str, width: usize) -> String {
    if UnicodeWidthStr::width(path) <= width {
        return path.to_string();
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() <= 2 {
        return path.to_string();
    }
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            if index == 0 || index + 1 == parts.len() || part.is_empty() {
                (*part).to_string()
            } else {
                part.graphemes(true).next().unwrap_or_default().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn truncate_styled_line(line: Line<'static>, width: usize) -> Line<'static> {
    if line.width() <= width {
        return line;
    }
    if width == 0 {
        return Line::default();
    }
    let ellipsis_width = usize::from(width > 1);
    let content_width = width.saturating_sub(ellipsis_width);
    let mut used: usize = 0;
    let mut spans = Vec::new();
    'outer: for span in line.spans {
        let mut content = String::new();
        for grapheme in span.content.graphemes(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if used.saturating_add(grapheme_width) > content_width {
                if !content.is_empty() {
                    spans.push(Span::styled(content, span.style));
                }
                break 'outer;
            }
            used = used.saturating_add(grapheme_width);
            content.push_str(grapheme);
        }
        if !content.is_empty() {
            spans.push(Span::styled(content, span.style));
        }
    }
    if ellipsis_width > 0 {
        spans.push(Span::styled("…", Style::default().fg(MUTED)));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> ClaudeStatusLineData {
        ClaudeStatusLineData {
            model: "GPT-5.4".to_string(),
            reasoning: Some("high".to_string()),
            current_dir: "~/developer/codex".to_string(),
            git_branch: Some("main".to_string()),
            context_used_percent: Some(31),
            session_input_tokens: 842_000,
            session_output_tokens: 96_000,
            account_usage: Some(AccountUsageSummary {
                daily: Some(1_200_000),
                weekly: Some(4_800_000),
                monthly: Some(18_600_000),
                total: Some(42_100_000),
            }),
            five_hour_limit: Some("5h 18% reset 03:25".to_string()),
            weekly_limit: Some("Limit/week 42%".to_string()),
        }
    }

    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn wide_layout_uses_two_lines_with_all_metrics() {
        let lines = render_claude_status_line(&data(), 180);
        assert_eq!(lines.len(), 2);
        assert_eq!(text(&lines[0]), "GPT-5.4 · high │ ~/developer/codex │ main");
        let metrics = text(&lines[1]);
        for expected in [
            "Ctx 31%",
            "Session ↑842K ↓96K",
            "Today 1.2M",
            "Total 42.1M",
            "5h 18% reset 03:25",
        ] {
            assert!(metrics.contains(expected), "missing {expected}: {metrics}");
        }
    }

    #[test]
    fn narrow_layout_uses_three_lines_and_keeps_account_metrics() {
        let lines = render_claude_status_line(&data(), 100);
        assert_eq!(lines.len(), 3);
        assert!(text(&lines[0]).contains("~/d/codex"));
        let account = text(&lines[2]);
        for expected in ["Today 1.2M", "Week 4.8M", "Month 18.6M", "Total 42.1M"] {
            assert!(account.contains(expected), "missing {expected}: {account}");
        }
    }

    #[test]
    fn ultra_narrow_layout_truncates_each_line_safely() {
        let lines = render_claude_status_line(&data(), 60);
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| line.width() <= 60));
    }

    #[test]
    fn context_bar_has_exactly_ten_cells() {
        assert_eq!(context_bar(31), "▓▓▓░░░░░░░");
    }

    #[test]
    fn missing_account_usage_uses_dashes() {
        let mut missing = data();
        missing.account_usage = None;
        let lines = render_claude_status_line(&missing, 100);
        assert_eq!(text(&lines[2]), "Today — │ Week — │ Month — │ Total —");
    }

    #[test]
    fn semantic_fields_use_approved_colors() {
        let lines = render_claude_status_line(&data(), 180);
        assert_eq!(lines[0].spans[0].content, "GPT-5.4");
        assert_eq!(lines[0].spans[0].style.fg, Some(MODEL));
        assert!(
            lines[0].spans[0]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        assert_eq!(lines[0].spans[1].content, " · high");
        assert_eq!(lines[0].spans[1].style.fg, Some(MODEL));
        assert!(
            !lines[0].spans[1]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        assert_eq!(lines[0].spans[3].style.fg, Some(PATH));
        assert_eq!(lines[1].spans[0].style.fg, Some(CONTEXT));
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.style.fg == Some(INPUT))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.style.fg == Some(OUTPUT))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.style.fg == Some(TOTAL))
        );
    }

    #[test]
    fn narrow_layout_snapshot() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::widgets::Paragraph;

        let lines = render_claude_status_line(&data(), 100);
        let mut terminal = Terminal::new(TestBackend::new(100, 3)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget(Paragraph::new(lines), frame.area()))
            .expect("render status line");

        insta::assert_snapshot!("claude_status_line_narrow", terminal.backend());
    }
}
