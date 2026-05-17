use std::{
    io::{self, Stdout},
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use demo_server::{SegmentObservation, TransferReport, TransferSummary};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
};

const CANONICAL_SHOWCASE_REPORT: &str = "live_realtime_amc_preview_lte_constrained_report.json";

pub fn run_report_replay(report: &TransferReport, speed: f64) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let replay_result = run_app(&mut terminal, report, speed.max(0.1));
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    replay_result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    report: &TransferReport,
    speed: f64,
) -> Result<()> {
    let media = report
        .observations
        .iter()
        .filter(|observation| matches!(observation.kind, demo_server::SegmentKind::Media))
        .collect::<Vec<_>>();
    let total_steps = media.len().max(1);
    let mut step = 0usize;
    let started = Instant::now();
    let report_file = report_file_name(report);
    let canonical_showcase = is_canonical_showcase(report);

    loop {
        let elapsed_ms = (started.elapsed().as_millis() as f64 * speed) as u64;
        while step + 1 < media.len() && media[step + 1].server_receive_elapsed_ms <= elapsed_ms {
            step += 1;
        }

        terminal.draw(|frame| {
            let areas = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(8),
                    Constraint::Length(9),
                    Constraint::Min(7),
                ])
                .split(frame.area());

            let current = media.get(step).copied().or_else(|| media.first().copied());
            let progress = ((step + 1) as f64 / total_steps as f64 * 100.0).round() as u16;
            let gauge = Gauge::default()
                .block(
                    Block::default()
                        .title("Replay Progress")
                        .borders(Borders::ALL),
                )
                .gauge_style(Style::default().fg(Color::Cyan))
                .percent(progress);
            frame.render_widget(gauge, areas[0]);

            let top = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(areas[1]);
            frame.render_widget(summary_panel(report, current, &report_file), top[0]);
            frame.render_widget(
                status_panel(report, current, canonical_showcase, step, total_steps, elapsed_ms),
                top[1],
            );

            let middle = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
                .split(areas[2]);
            frame.render_widget(current_observation_panel(current), middle[0]);
            frame.render_widget(runtime_telemetry_panel(current), middle[1]);

            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(34),
                    Constraint::Percentage(33),
                    Constraint::Percentage(33),
                ])
                .split(areas[3]);

            let utility_points = media
                .iter()
                .take(step + 1)
                .map(|observation| {
                    observation
                        .runtime_utility
                        .as_ref()
                        .map(|runtime| (runtime.utility_score * 1000.0).round() as u64)
                        .unwrap_or(0)
                })
                .collect::<Vec<_>>();
            let latency_points = media
                .iter()
                .take(step + 1)
                .map(|observation| delivery_latency_ms(observation))
                .collect::<Vec<_>>();
            let lateness_points = media
                .iter()
                .take(step + 1)
                .map(|observation| observation.lateness_ms.max(0) as u64)
                .collect::<Vec<_>>();

            frame.render_widget(
                Sparkline::default()
                    .block(
                        Block::default()
                            .title("Utility Score x1000")
                            .borders(Borders::ALL),
                    )
                    .data(&utility_points)
                    .style(Style::default().fg(Color::Green)),
                bottom[0],
            );
            frame.render_widget(
                Sparkline::default()
                    .block(
                        Block::default()
                            .title("Delivery Latency ms")
                            .borders(Borders::ALL),
                    )
                    .data(&latency_points)
                    .style(Style::default().fg(Color::Yellow)),
                bottom[1],
            );
            frame.render_widget(
                Sparkline::default()
                    .block(
                        Block::default()
                            .title("Deadline Miss ms")
                            .borders(Borders::ALL),
                    )
                    .data(&lateness_points)
                    .style(Style::default().fg(Color::Red)),
                bottom[2],
            );
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
            }
        }

        if step + 1 >= media.len() && started.elapsed() > Duration::from_millis(500) {
            // Leave the final frame on screen until the operator quits.
        }
    }

    Ok(())
}

fn summary_panel(
    report: &TransferReport,
    current: Option<&SegmentObservation>,
    report_file: &str,
) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(vec![
            label_span("asset "),
            Span::raw(report.summary.asset_name.clone()),
            Span::raw("  "),
            label_span("controller "),
            Span::raw(format!("{:?}", report.summary.baseline_controller)),
            Span::raw("  "),
            label_span("mode "),
            Span::raw(current.map(observation_mode_label).unwrap_or("unknown")),
        ]),
        Line::from(vec![label_span("report "), Span::raw(report_file.to_string())]),
        Line::from(vec![
            label_span("media "),
            Span::raw(format!(
                "{} / {} useful ({:.1}%)",
                report.summary.useful_media_segments,
                report.summary.media_segments_received,
                ratio_percent(
                    report.summary.useful_media_segments,
                    report.summary.media_segments_received,
                )
            )),
        ]),
        Line::from(vec![
            label_span("late "),
            Span::raw(format!(
                "{} / {} ({:.1}%)",
                report.summary.late_media_segments,
                report.summary.media_segments_received,
                ratio_percent(
                    report.summary.late_media_segments,
                    report.summary.media_segments_received,
                )
            )),
            Span::raw("  "),
            label_span("payload "),
            Span::raw(format!("{:.2} MiB", bytes_to_mib(report.summary.total_payload_bytes))),
        ]),
        Line::from(vec![
            label_span("utility samples "),
            Span::raw(report.summary.amc_runtime_samples.to_string()),
            Span::raw("  "),
            label_span("utility range "),
            Span::raw(summary_utility_range(&report.summary)),
        ]),
    ])
    .block(
        Block::default()
            .title("Transfer Summary")
            .borders(Borders::ALL),
    )
}

fn status_panel(
    report: &TransferReport,
    current: Option<&SegmentObservation>,
    canonical_showcase: bool,
    step: usize,
    total_steps: usize,
    elapsed_ms: u64,
) -> Paragraph<'static> {
    let (showcase_label, showcase_color) = if canonical_showcase {
        ("canonical showcase", Color::Green)
    } else {
        ("support-only report", Color::Yellow)
    };
    let (deadline_label, deadline_color) = current
        .map(|observation| deadline_status(observation.lateness_ms))
        .unwrap_or(("n/a".to_string(), Color::DarkGray));
    let current_latency = current.map(delivery_latency_ms).unwrap_or_default();
    let current_age = current.map(age_of_information_ms).unwrap_or_default();
    let current_useful = current.map(|observation| observation.useful).unwrap_or(false);

    Paragraph::new(vec![
        Line::from(vec![
            label_span("evidence "),
            Span::styled(showcase_label, Style::default().fg(showcase_color)),
        ]),
        Line::from(vec![
            label_span("step "),
            Span::raw(format!("{} / {}", step + 1, total_steps)),
            Span::raw("  "),
            label_span("elapsed "),
            Span::raw(format!("{} ms", elapsed_ms)),
        ]),
        Line::from(vec![
            label_span("current usefulness "),
            Span::styled(
                if current_useful { "useful" } else { "late / useless" },
                Style::default().fg(if current_useful { Color::Green } else { Color::Red }),
            ),
        ]),
        Line::from(vec![
            label_span("deadline status "),
            Span::styled(deadline_label, Style::default().fg(deadline_color)),
        ]),
        Line::from(vec![
            label_span("delivery latency "),
            Span::raw(format!("{} ms", current_latency)),
            Span::raw("  "),
            label_span("age of information "),
            Span::raw(format!("{} ms", current_age)),
        ]),
        Line::from(vec![
            label_span("quit "),
            Span::raw("q or Esc"),
            Span::raw("  "),
            label_span("path "),
            Span::raw(report.summary.report_path.clone()),
        ]),
    ])
    .block(Block::default().title("Showcase Status").borders(Borders::ALL))
}

fn current_observation_panel(current: Option<&SegmentObservation>) -> Paragraph<'static> {
    let lines = if let Some(observation) = current {
        vec![
            Line::from(vec![
                label_span("sequence "),
                Span::raw(observation.sequence.to_string()),
                Span::raw("  "),
                label_span("kind "),
                Span::raw(observation_kind_label(observation)),
            ]),
            Line::from(vec![
                label_span("start "),
                Span::raw(format!("{} ms", observation.start_time_ms)),
                Span::raw("  "),
                label_span("duration "),
                Span::raw(format!("{} ms", observation.duration_ms)),
                Span::raw("  "),
                label_span("deadline "),
                Span::raw(format!("{} ms", observation.deadline_ms)),
            ]),
            Line::from(vec![
                label_span("client send "),
                Span::raw(format!("{} ms", observation.client_send_elapsed_ms)),
                Span::raw("  "),
                label_span("server receive "),
                Span::raw(format!("{} ms", observation.server_receive_elapsed_ms)),
            ]),
            Line::from(vec![
                label_span("payload "),
                Span::raw(format!("{} bytes", observation.payload_len)),
                Span::raw("  "),
                label_span("lateness "),
                Span::raw(format!("{} ms", observation.lateness_ms)),
            ]),
            Line::from(vec![
                label_span("path "),
                Span::raw(observation.segment_path.clone()),
            ]),
        ]
    } else {
        vec![Line::from("no media observations available")]
    };

    Paragraph::new(lines).block(
        Block::default()
            .title("Current Observation")
            .borders(Borders::ALL),
    )
}

fn runtime_telemetry_panel(current: Option<&SegmentObservation>) -> Paragraph<'static> {
    let lines = if let Some(observation) = current {
        match observation.runtime_utility.as_ref() {
            Some(runtime) => vec![
                Line::from(vec![
                    label_span("traffic class "),
                    Span::raw(format!("{:?}", runtime.traffic_class)),
                    Span::raw("  "),
                    label_span("importance "),
                    Span::raw(format!("{:?}", runtime.importance)),
                ]),
                Line::from(vec![
                    label_span("dependency depth "),
                    Span::raw(runtime.dependency_depth.to_string()),
                    Span::raw("  "),
                    label_span("dependency ready "),
                    Span::styled(
                        if runtime.dependency_ready {
                            "true"
                        } else {
                            "false"
                        },
                        Style::default().fg(if runtime.dependency_ready {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                ]),
                Line::from(vec![
                    label_span("queue delay "),
                    Span::raw(format!("{} ms", runtime.queue_delay_ms)),
                    Span::raw("  "),
                    label_span("estimated rtt "),
                    Span::raw(format!("{} ms", runtime.estimated_rtt_ms)),
                ]),
                Line::from(vec![
                    label_span("utility score "),
                    Span::raw(format!("{:.4}", runtime.utility_score)),
                ]),
                Line::from(vec![
                    label_span("ack gain "),
                    Span::raw(format!("{:.3}", runtime.ack_gain)),
                    Span::raw("  "),
                    label_span("loss factor "),
                    Span::raw(format!("{:.3}", runtime.loss_reduction_factor)),
                ]),
            ],
            None => vec![
                Line::from("runtime utility telemetry is not present in this report"),
                Line::from("the Phase 8 showcase expects AMC runtime samples for all segments"),
            ],
        }
    } else {
        vec![Line::from("no media observations available")]
    };

    Paragraph::new(lines).block(
        Block::default()
            .title("Runtime Utility Telemetry")
            .borders(Borders::ALL),
    )
}

fn report_file_name(report: &TransferReport) -> String {
    Path::new(&report.summary.report_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&report.summary.report_path)
        .to_string()
}

fn is_canonical_showcase(report: &TransferReport) -> bool {
    report_file_name(report) == CANONICAL_SHOWCASE_REPORT
}

fn summary_utility_range(summary: &TransferSummary) -> String {
    match (
        summary.min_runtime_utility_score,
        summary.max_runtime_utility_score,
    ) {
        (Some(minimum), Some(maximum)) => format!("{minimum:.4} .. {maximum:.4}"),
        _ => "n/a".to_string(),
    }
}

fn delivery_latency_ms(observation: &SegmentObservation) -> u64 {
    observation
        .server_receive_elapsed_ms
        .saturating_sub(observation.client_send_elapsed_ms)
}

fn age_of_information_ms(observation: &SegmentObservation) -> u64 {
    observation
        .server_receive_elapsed_ms
        .saturating_sub(observation.start_time_ms)
}

fn ratio_percent(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64 * 100.0
    }
}

fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn deadline_status(lateness_ms: i64) -> (String, Color) {
    if lateness_ms > 0 {
        (format!("late by {} ms", lateness_ms), Color::Red)
    } else {
        (format!("on time by {} ms", lateness_ms.unsigned_abs()), Color::Green)
    }
}

fn observation_kind_label(observation: &SegmentObservation) -> &'static str {
    match observation.kind {
        demo_server::SegmentKind::Init => "init",
        demo_server::SegmentKind::Media => "media",
    }
}

fn observation_mode_label(observation: &SegmentObservation) -> &'static str {
    match observation.mode {
        demo_server::ReplayMode::Vod => "vod",
        demo_server::ReplayMode::Live => "live",
    }
}

fn label_span(label: &'static str) -> Span<'static> {
    Span::styled(label, Style::default().add_modifier(Modifier::BOLD))
}

#[cfg(test)]
mod tests {
    use super::{CANONICAL_SHOWCASE_REPORT, deadline_status, is_canonical_showcase};

    fn sample_report(report_path: &str) -> demo_server::TransferReport {
        demo_server::TransferReport {
            metadata: None,
            summary: demo_server::TransferSummary {
                asset_name: "asset".to_string(),
                baseline_controller: demo_client::BaselineController::AmcPreview,
                segments_received: 1,
                media_segments_received: 1,
                total_payload_bytes: 10,
                useful_media_segments: 1,
                late_media_segments: 0,
                max_observed_lateness_ms: 0,
                amc_runtime_samples: 1,
                max_runtime_utility_score: Some(0.1),
                min_runtime_utility_score: Some(0.1),
                report_path: report_path.to_string(),
            },
            observations: Vec::new(),
        }
    }

    #[test]
    fn canonical_showcase_detection_matches_expected_report_name() {
        let report = sample_report(&format!("results/raw/harness/{}", CANONICAL_SHOWCASE_REPORT));
        assert!(is_canonical_showcase(&report));
    }

    #[test]
    fn noncanonical_reports_are_marked_support_only() {
        let report = sample_report("results/raw/harness/live_realtime_amc_preview_report.json");
        assert!(!is_canonical_showcase(&report));
    }

    #[test]
    fn deadline_status_distinguishes_late_and_ontime_segments() {
        let (late, _) = deadline_status(42);
        let (ontime, _) = deadline_status(-120);
        assert_eq!(late, "late by 42 ms");
        assert_eq!(ontime, "on time by 120 ms");
    }
}
