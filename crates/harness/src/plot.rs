use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use plotters::prelude::*;
use tokio::fs;

use crate::analysis::{ComparisonRow, SuiteComparisonExport};

struct MetricSpec {
    file_stem: &'static str,
    title: &'static str,
    unit: &'static str,
    value: fn(&ComparisonRow) -> Option<f64>,
}

pub async fn plot_comparison_export(
    comparison_path: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let bytes = fs::read(comparison_path)
        .await
        .with_context(|| format!("failed to read {}", comparison_path.display()))?;
    let export: SuiteComparisonExport = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", comparison_path.display()))?;

    fs::create_dir_all(output_dir)
        .await
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    let mut outputs = Vec::new();
    outputs.extend(render_overview_charts(output_dir, &export)?);
    outputs.extend(render_matrix_charts(output_dir, &export)?);

    Ok(outputs)
}

fn render_overview_charts(
    output_dir: &Path,
    export: &SuiteComparisonExport,
) -> Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();
    outputs.push(render_metric_chart(
        output_dir,
        "useful_media_ratio.svg",
        "Useful Media Ratio",
        &export
            .rows
            .iter()
            .map(|row| (label_for_row(row), row.useful_media_ratio * 100.0))
            .collect::<Vec<_>>(),
        "percent",
    )?);
    outputs.push(render_metric_chart(
        output_dir,
        "deadline_miss_rate.svg",
        "Deadline Miss Rate",
        &export
            .rows
            .iter()
            .map(|row| (label_for_row(row), row.deadline_miss_rate * 100.0))
            .collect::<Vec<_>>(),
        "percent",
    )?);
    outputs.push(render_metric_chart(
        output_dir,
        "throughput_mbps.svg",
        "Throughput Mbps",
        &export
            .rows
            .iter()
            .map(|row| (label_for_row(row), row.throughput_mbps))
            .collect::<Vec<_>>(),
        "mbps",
    )?);

    let live_aoi = export
        .rows
        .iter()
        .filter_map(|row| {
            row.average_age_of_information_ms
                .map(|value| (label_for_row(row), value))
        })
        .collect::<Vec<_>>();
    if !live_aoi.is_empty() {
        outputs.push(render_metric_chart(
            output_dir,
            "live_average_aoi_ms.svg",
            "Live Average Age of Information",
            &live_aoi,
            "ms",
        )?);
    }

    let vod_startup_delay = export
        .rows
        .iter()
        .filter_map(|row| {
            row.vod_startup_delay_ms
                .map(|value| (label_for_row(row), value as f64))
        })
        .collect::<Vec<_>>();
    if !vod_startup_delay.is_empty() {
        outputs.push(render_metric_chart(
            output_dir,
            "vod_startup_delay_ms.svg",
            "VOD Startup Delay",
            &vod_startup_delay,
            "ms",
        )?);
    }

    let vod_rebuffer_ratio = export
        .rows
        .iter()
        .filter_map(|row| {
            row.vod_rebuffer_ratio
                .map(|value| (label_for_row(row), value * 100.0))
        })
        .collect::<Vec<_>>();
    if !vod_rebuffer_ratio.is_empty() {
        outputs.push(render_metric_chart(
            output_dir,
            "vod_rebuffer_ratio.svg",
            "VOD Rebuffer Ratio",
            &vod_rebuffer_ratio,
            "percent",
        )?);
    }

    Ok(outputs)
}

fn render_matrix_charts(output_dir: &Path, export: &SuiteComparisonExport) -> Result<Vec<PathBuf>> {
    let mut outputs = Vec::new();
    let metrics = [
        MetricSpec {
            file_stem: "useful_media_ratio",
            title: "Useful Media Ratio",
            unit: "percent",
            value: metric_useful_media_ratio,
        },
        MetricSpec {
            file_stem: "deadline_miss_rate",
            title: "Deadline Miss Rate",
            unit: "percent",
            value: metric_deadline_miss_rate,
        },
        MetricSpec {
            file_stem: "throughput_mbps",
            title: "Throughput",
            unit: "mbps",
            value: metric_throughput_mbps,
        },
        MetricSpec {
            file_stem: "average_delivery_latency_ms",
            title: "Average Delivery Latency",
            unit: "ms",
            value: metric_average_delivery_latency_ms,
        },
        MetricSpec {
            file_stem: "average_jitter_ms",
            title: "Average Jitter",
            unit: "ms",
            value: metric_average_jitter_ms,
        },
        MetricSpec {
            file_stem: "average_age_of_information_ms",
            title: "Average Age of Information",
            unit: "ms",
            value: metric_average_age_of_information_ms,
        },
        MetricSpec {
            file_stem: "vod_startup_delay_ms",
            title: "VOD Startup Delay",
            unit: "ms",
            value: metric_vod_startup_delay_ms,
        },
        MetricSpec {
            file_stem: "vod_rebuffer_ratio",
            title: "VOD Rebuffer Ratio",
            unit: "percent",
            value: metric_vod_rebuffer_ratio,
        },
    ];

    let mut mode_paces = Vec::new();
    for row in &export.rows {
        let key = (mode_key(row.mode), pace_key(row.pace));
        if mode_paces
            .iter()
            .any(|(mode_name, pace_name, _, _)| *mode_name == key.0 && *pace_name == key.1)
        {
            continue;
        }
        mode_paces.push((key.0, key.1, row.mode, row.pace));
    }

    for (_, _, mode, pace) in mode_paces {
        for metric in &metrics {
            if let Some(output) =
                render_grouped_metric_chart(output_dir, export, mode, pace, metric)?
            {
                outputs.push(output);
            }
        }
    }

    Ok(outputs)
}

fn render_metric_chart(
    output_dir: &Path,
    file_name: &str,
    title: &str,
    rows: &[(String, f64)],
    unit: &str,
) -> Result<PathBuf> {
    let output_path = output_dir.join(file_name);
    {
        let backend = SVGBackend::new(&output_path, (1600, 900));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)?;

        let max_value = rows
            .iter()
            .map(|(_, value)| *value)
            .fold(0.0_f64, f64::max)
            .max(1.0);

        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 32))
            .margin(20)
            .x_label_area_size(80)
            .y_label_area_size(90)
            .build_cartesian_2d(0..rows.len(), 0f64..(max_value * 1.15))?;

        chart
            .configure_mesh()
            .y_desc(unit)
            .x_labels(rows.len().min(20))
            .x_label_formatter(&|index| {
                rows.get(*index)
                    .map(|(label, _)| label.clone())
                    .unwrap_or_default()
            })
            .x_label_style(("sans-serif", 14).into_font())
            .axis_desc_style(("sans-serif", 18))
            .draw()?;

        chart.draw_series(rows.iter().enumerate().map(|(index, (_, value))| {
            Rectangle::new([(index, 0.0), (index + 1, *value)], BLUE.mix(0.6).filled())
        }))?;

        root.present()?;
    }
    Ok(output_path)
}

fn render_grouped_metric_chart(
    output_dir: &Path,
    export: &SuiteComparisonExport,
    mode: demo_client::ReplayMode,
    pace: demo_client::Pace,
    metric: &MetricSpec,
) -> Result<Option<PathBuf>> {
    let rows = export
        .rows
        .iter()
        .filter(|row| mode_key(row.mode) == mode_key(mode) && pace_key(row.pace) == pace_key(pace))
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Ok(None);
    }

    let scenarios = scenario_order(export, mode, pace, &rows);
    let controllers = controller_order(export, mode, pace, &rows);
    let values = rows
        .iter()
        .filter_map(|row| {
            (metric.value)(row).map(|value| {
                (
                    (
                        row.network_scenario.clone(),
                        controller_key(row.controller).to_string(),
                    ),
                    value,
                )
            })
        })
        .collect::<HashMap<_, _>>();

    if values.is_empty() || scenarios.is_empty() || controllers.is_empty() {
        return Ok(None);
    }

    let file_name = format!(
        "{}_{}_{}.svg",
        mode_key(mode),
        pace_key(pace),
        metric.file_stem
    );
    let output_path = output_dir.join(file_name);
    let max_value = values.values().copied().fold(0.0_f64, f64::max).max(1.0);

    {
        let backend = SVGBackend::new(&output_path, (1800, 960));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)?;

        let x_end = scenarios.len() as f64 - 0.5;
        let mut chart = ChartBuilder::on(&root)
            .caption(
                format!(
                    "{} {} by Scenario ({})",
                    mode_title(mode),
                    metric.title,
                    pace_key(pace)
                ),
                ("sans-serif", 32),
            )
            .margin(20)
            .x_label_area_size(80)
            .y_label_area_size(90)
            .build_cartesian_2d(-0.5f64..x_end, 0f64..(max_value * 1.15))?;

        chart
            .configure_mesh()
            .x_desc("network scenario")
            .y_desc(metric.unit)
            .x_labels(scenarios.len())
            .x_label_formatter(&|value| {
                let index = value.round() as isize;
                if index < 0 {
                    return String::new();
                }
                scenarios
                    .get(index as usize)
                    .map(|scenario| humanize_scenario(scenario))
                    .unwrap_or_default()
            })
            .x_label_style(("sans-serif", 18).into_font())
            .axis_desc_style(("sans-serif", 20))
            .light_line_style(TRANSPARENT)
            .draw()?;

        let group_width = 0.8;
        let bar_width = group_width / controllers.len() as f64;
        for (controller_index, controller) in controllers.iter().enumerate() {
            let color = controller_color(controller);
            chart
                .draw_series(scenarios.iter().enumerate().filter_map(
                    |(scenario_index, scenario)| {
                        let value = values.get(&(scenario.clone(), controller.clone()))?;
                        let center = scenario_index as f64;
                        let x0 = center - (group_width / 2.0) + controller_index as f64 * bar_width;
                        let x1 = x0 + bar_width * 0.9;
                        Some(Rectangle::new([(x0, 0.0), (x1, *value)], color.filled()))
                    },
                ))?
                .label(controller_title(controller))
                .legend(move |(x, y)| {
                    Rectangle::new([(x, y - 6), (x + 16, y + 6)], color.filled())
                });
        }

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .background_style(WHITE.mix(0.85))
            .border_style(BLACK.mix(0.2))
            .label_font(("sans-serif", 18))
            .draw()?;

        root.present()?;
    }

    Ok(Some(output_path))
}

fn scenario_order(
    export: &SuiteComparisonExport,
    mode: demo_client::ReplayMode,
    pace: demo_client::Pace,
    rows: &[&ComparisonRow],
) -> Vec<String> {
    let from_groups = export
        .matrix_groups
        .iter()
        .filter(|group| {
            mode_key(group.mode) == mode_key(mode) && pace_key(group.pace) == pace_key(pace)
        })
        .map(|group| group.network_scenario.clone())
        .collect::<Vec<_>>();
    if !from_groups.is_empty() {
        return from_groups;
    }

    let mut scenarios = rows
        .iter()
        .map(|row| row.network_scenario.clone())
        .collect::<Vec<_>>();
    scenarios.sort();
    scenarios.dedup();
    scenarios
}

fn controller_order(
    export: &SuiteComparisonExport,
    mode: demo_client::ReplayMode,
    pace: demo_client::Pace,
    rows: &[&ComparisonRow],
) -> Vec<String> {
    let mut controllers = export
        .matrix_groups
        .iter()
        .filter(|group| {
            mode_key(group.mode) == mode_key(mode) && pace_key(group.pace) == pace_key(pace)
        })
        .flat_map(|group| group.expected_controllers.iter().cloned())
        .collect::<Vec<_>>();
    if controllers.is_empty() {
        controllers = rows
            .iter()
            .map(|row| controller_key(row.controller).to_string())
            .collect();
    }

    controllers.sort_by_key(|controller| controller_rank(controller));
    controllers.dedup();
    controllers
}

fn metric_useful_media_ratio(row: &ComparisonRow) -> Option<f64> {
    Some(row.useful_media_ratio * 100.0)
}

fn metric_deadline_miss_rate(row: &ComparisonRow) -> Option<f64> {
    Some(row.deadline_miss_rate * 100.0)
}

fn metric_throughput_mbps(row: &ComparisonRow) -> Option<f64> {
    Some(row.throughput_mbps)
}

fn metric_average_delivery_latency_ms(row: &ComparisonRow) -> Option<f64> {
    Some(row.average_delivery_latency_ms)
}

fn metric_average_jitter_ms(row: &ComparisonRow) -> Option<f64> {
    Some(row.average_jitter_ms)
}

fn metric_average_age_of_information_ms(row: &ComparisonRow) -> Option<f64> {
    row.average_age_of_information_ms
}

fn metric_vod_startup_delay_ms(row: &ComparisonRow) -> Option<f64> {
    row.vod_startup_delay_ms.map(|value| value as f64)
}

fn metric_vod_rebuffer_ratio(row: &ComparisonRow) -> Option<f64> {
    row.vod_rebuffer_ratio.map(|value| value * 100.0)
}

fn mode_key(mode: demo_client::ReplayMode) -> &'static str {
    match mode {
        demo_client::ReplayMode::Vod => "vod",
        demo_client::ReplayMode::Live => "live",
    }
}

fn mode_title(mode: demo_client::ReplayMode) -> &'static str {
    match mode {
        demo_client::ReplayMode::Vod => "VOD",
        demo_client::ReplayMode::Live => "Live",
    }
}

fn pace_key(pace: demo_client::Pace) -> &'static str {
    match pace {
        demo_client::Pace::Immediate => "immediate",
        demo_client::Pace::Realtime => "realtime",
    }
}

fn controller_key(controller: demo_client::BaselineController) -> &'static str {
    match controller {
        demo_client::BaselineController::NewReno => "new_reno",
        demo_client::BaselineController::Cubic => "cubic",
        demo_client::BaselineController::Bbr => "bbr",
        demo_client::BaselineController::AmcPreview => "amc_preview",
    }
}

fn controller_title(controller: &str) -> &'static str {
    match controller {
        "new_reno" => "NewReno",
        "cubic" => "Cubic",
        "bbr" => "BBR",
        "amc_preview" => "AMC Preview",
        _ => "Unknown",
    }
}

fn controller_rank(controller: &str) -> usize {
    match controller {
        "new_reno" => 0,
        "cubic" => 1,
        "bbr" => 2,
        "amc_preview" => 3,
        _ => usize::MAX,
    }
}

fn controller_color(controller: &str) -> RGBAColor {
    match controller {
        "new_reno" => RGBColor(32, 86, 173).mix(0.75),
        "cubic" => RGBColor(31, 160, 136).mix(0.75),
        "bbr" => RGBColor(231, 111, 81).mix(0.75),
        "amc_preview" => RGBColor(185, 77, 148).mix(0.8),
        _ => BLACK.mix(0.7),
    }
}

fn humanize_scenario(scenario: &str) -> String {
    scenario.replace('_', " ")
}

fn label_for_row(row: &crate::analysis::ComparisonRow) -> String {
    format!(
        "{}:{}:{}",
        row.network_scenario,
        match row.mode {
            demo_client::ReplayMode::Vod => "vod",
            demo_client::ReplayMode::Live => "live",
        },
        controller_key(row.controller)
    )
}
