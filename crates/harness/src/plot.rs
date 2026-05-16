use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use plotters::prelude::*;
use tokio::fs;

use crate::analysis::SuiteComparisonExport;

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
    outputs.push(render_metric_chart(
        output_dir,
        "useful_media_ratio.svg",
        "Useful Media Ratio",
        &export.rows.iter().map(|row| (label_for_row(row), row.useful_media_ratio * 100.0)).collect::<Vec<_>>(),
        "percent",
    )?);
    outputs.push(render_metric_chart(
        output_dir,
        "deadline_miss_rate.svg",
        "Deadline Miss Rate",
        &export.rows.iter().map(|row| (label_for_row(row), row.deadline_miss_rate * 100.0)).collect::<Vec<_>>(),
        "percent",
    )?);
    outputs.push(render_metric_chart(
        output_dir,
        "throughput_mbps.svg",
        "Throughput Mbps",
        &export.rows.iter().map(|row| (label_for_row(row), row.throughput_mbps)).collect::<Vec<_>>(),
        "mbps",
    )?);

    let live_aoi = export
        .rows
        .iter()
        .filter_map(|row| row.average_age_of_information_ms.map(|value| (label_for_row(row), value)))
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
            .x_label_formatter(&|index| rows.get(*index).map(|(label, _)| label.clone()).unwrap_or_default())
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

fn label_for_row(row: &crate::analysis::ComparisonRow) -> String {
    format!(
        "{}:{}:{}",
        row.network_scenario,
        match row.mode {
            demo_client::ReplayMode::Vod => "vod",
            demo_client::ReplayMode::Live => "live",
        },
        match row.controller {
            demo_client::BaselineController::NewReno => "new_reno",
            demo_client::BaselineController::Cubic => "cubic",
            demo_client::BaselineController::Bbr => "bbr",
            demo_client::BaselineController::AmcPreview => "amc_preview",
        }
    )
}