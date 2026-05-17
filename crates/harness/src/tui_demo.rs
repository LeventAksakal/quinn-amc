use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use demo_server::TransferReport;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
};

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
        .filter(|obs| matches!(obs.kind, demo_server::SegmentKind::Media))
        .collect::<Vec<_>>();
    let total_steps = media.len().max(1);
    let mut step = 0usize;
    let started = Instant::now();

    loop {
        let elapsed_ms = (started.elapsed().as_millis() as f64 * speed) as u64;
        while step + 1 < media.len() && media[step + 1].server_receive_elapsed_ms <= elapsed_ms {
            step += 1;
        }

        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(7),
                    Constraint::Length(7),
                    Constraint::Min(5),
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
            frame.render_widget(gauge, chunks[0]);

            let summary = Paragraph::new(vec![
                Line::from(format!("asset: {}", report.summary.asset_name)),
                Line::from(format!(
                    "controller: {:?}",
                    report.summary.baseline_controller
                )),
                Line::from(format!(
                    "segments: {} media: {}",
                    report.summary.segments_received, report.summary.media_segments_received
                )),
                Line::from(format!(
                    "useful: {} late: {}",
                    report.summary.useful_media_segments, report.summary.late_media_segments
                )),
                Line::from(format!(
                    "runtime utility samples: {}",
                    report.summary.amc_runtime_samples
                )),
            ])
            .block(
                Block::default()
                    .title("Transfer Summary")
                    .borders(Borders::ALL),
            );
            frame.render_widget(summary, chunks[1]);

            let detail_lines = if let Some(current) = current {
                vec![
                    Line::from(format!(
                        "sequence: {} useful: {} lateness_ms: {}",
                        current.sequence, current.useful, current.lateness_ms
                    )),
                    Line::from(format!(
                        "delivery_latency_ms: {}",
                        current
                            .server_receive_elapsed_ms
                            .saturating_sub(current.client_send_elapsed_ms)
                    )),
                    Line::from(format!(
                        "age_of_information_ms: {}",
                        current
                            .server_receive_elapsed_ms
                            .saturating_sub(current.start_time_ms)
                    )),
                    Line::from(match current.runtime_utility.as_ref() {
                        Some(runtime) => format!(
                            "utility_score: {:.4} ack_gain: {:.3} loss_factor: {:.3}",
                            runtime.utility_score, runtime.ack_gain, runtime.loss_reduction_factor
                        ),
                        None => "utility_score: n/a ack_gain: n/a loss_factor: n/a".to_string(),
                    }),
                    Line::from(format!("segment_path: {}", current.segment_path)),
                ]
            } else {
                vec![Line::from("no media observations available")]
            };
            let detail = Paragraph::new(detail_lines).block(
                Block::default()
                    .title("Current Observation")
                    .borders(Borders::ALL),
            );
            frame.render_widget(detail, chunks[2]);

            let utility_points = media
                .iter()
                .take(step + 1)
                .map(|obs| {
                    obs.runtime_utility
                        .as_ref()
                        .map(|runtime| (runtime.utility_score * 1000.0).round() as u64)
                        .unwrap_or(0)
                })
                .collect::<Vec<_>>();
            let latency_points = media
                .iter()
                .take(step + 1)
                .map(|obs| {
                    obs.server_receive_elapsed_ms
                        .saturating_sub(obs.client_send_elapsed_ms)
                })
                .collect::<Vec<_>>();
            let bottom = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[3]);
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
