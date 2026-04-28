#!/usr/bin/env python

import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def percentile(values: list[int], fraction: float) -> int:
    if not values:
        raise RuntimeError("cannot compute percentile of empty list")
    index = round((len(values) - 1) * fraction)
    return sorted(values)[index]


def local_name(tag: str) -> str:
    return tag.split("}", 1)[-1]


def parse_required_int(attributes: dict[str, str], key: str) -> int:
    raw_value = attributes.get(key)
    if raw_value is None:
        raise RuntimeError(f"missing {key} in SegmentTemplate")
    try:
        value = int(raw_value)
    except ValueError as exc:
        raise RuntimeError(f"invalid integer for SegmentTemplate/{key}: {raw_value}") from exc
    if value <= 0:
        raise RuntimeError(f"SegmentTemplate/{key} must be positive, got {value}")
    return value


def parse_optional_positive_int(attributes: dict[str, str], key: str, default: int) -> int:
    raw_value = attributes.get(key)
    if raw_value is None:
        return default
    try:
        value = int(raw_value)
    except ValueError as exc:
        raise RuntimeError(f"invalid integer for SegmentTemplate/{key}: {raw_value}") from exc
    if value <= 0:
        raise RuntimeError(f"SegmentTemplate/{key} must be positive, got {value}")
    return value


def find_segment_template(root: ET.Element) -> ET.Element:
    templates = [element for element in root.iter() if local_name(element.tag) == "SegmentTemplate"]
    if not templates:
        raise RuntimeError("missing SegmentTemplate in MPD")
    if len(templates) != 1:
        raise RuntimeError(
            f"expected exactly one SegmentTemplate in MPD, found {len(templates)}"
        )
    return templates[0]


def require_relative_asset_path(raw_value: str, field_name: str) -> Path:
    if not raw_value:
        raise RuntimeError(f"SegmentTemplate/{field_name} must not be empty")
    path = Path(raw_value)
    if path.is_absolute() or ".." in path.parts:
        raise RuntimeError(
            f"SegmentTemplate/{field_name} must stay within the asset directory: {raw_value}"
        )
    return path


def compile_number_template(media_template: str) -> tuple[re.Pattern[str], str]:
    match = re.search(r"\$Number(?:%0(\d+)d)?\$", media_template)
    if not match:
        raise RuntimeError(
            "SegmentTemplate/media must contain a $Number$ or $Number%0Nd$ placeholder"
        )

    width = int(match.group(1)) if match.group(1) else 0
    if width <= 0:
        sequence_group = r"(?P<sequence>\d+)"
        formatter = "{sequence}"
    else:
        sequence_group = rf"(?P<sequence>\d{{{width}}})"
        formatter = f"{{sequence:0{width}d}}"

    pattern = (
        "^"
        + re.escape(media_template[: match.start()])
        + sequence_group
        + re.escape(media_template[match.end() :])
        + "$"
    )
    return re.compile(pattern), media_template[: match.start()] + formatter + media_template[match.end() :]


def collect_media_segments(asset_dir: Path, media_template: str, start_number: int) -> list[tuple[int, Path]]:
    matcher, formatter = compile_number_template(media_template)
    matched_segments: dict[int, Path] = {}
    unexpected_files: list[str] = []

    for media_file in sorted(asset_dir.glob("*.m4s")):
        segment_match = matcher.match(media_file.name)
        if segment_match is None:
            unexpected_files.append(media_file.name)
            continue

        sequence = int(segment_match.group("sequence"))
        if sequence in matched_segments:
            raise RuntimeError(
                f"duplicate segment sequence {sequence} for {matched_segments[sequence].name} and {media_file.name}"
            )
        matched_segments[sequence] = media_file

    if unexpected_files:
        raise RuntimeError(
            "found segment files that do not match SegmentTemplate/media: "
            + ", ".join(unexpected_files)
        )
    if not matched_segments:
        raise RuntimeError("no media segments matching SegmentTemplate/media were generated")

    sorted_sequences = sorted(matched_segments)
    expected_sequences = list(range(start_number, start_number + len(sorted_sequences)))
    if sorted_sequences != expected_sequences:
        missing = sorted(set(expected_sequences) - set(sorted_sequences))
        unexpected = sorted(set(sorted_sequences) - set(expected_sequences))
        details: list[str] = []
        if missing:
            details.append("missing sequences " + ", ".join(str(value) for value in missing))
        if unexpected:
            details.append("unexpected sequences " + ", ".join(str(value) for value in unexpected))
        raise RuntimeError("segment sequence numbering is not contiguous from startNumber: " + "; ".join(details))

    validated_segments: list[tuple[int, Path]] = []
    for sequence in expected_sequences:
        expected_name = formatter.format(sequence=sequence)
        media_file = matched_segments[sequence]
        if media_file.name != expected_name:
            raise RuntimeError(
                f"segment file name mismatch for sequence {sequence}: expected {expected_name}, found {media_file.name}"
            )
        validated_segments.append((sequence, media_file))

    return validated_segments


def parse_mpd(mpd_path: Path) -> ET.Element:
    if not mpd_path.is_file():
        raise RuntimeError(f"MPD file does not exist: {mpd_path}")
    try:
        return ET.parse(mpd_path).getroot()
    except ET.ParseError as exc:
        raise RuntimeError(f"failed to parse MPD XML {mpd_path}: {exc}") from exc


def validate_mpd_root(root: ET.Element) -> None:
    if local_name(root.tag) != "MPD":
        raise RuntimeError(f"expected MPD root element, found {local_name(root.tag)}")


def main() -> int:
    if len(sys.argv) != 4:
        raise SystemExit("usage: build_replay_manifest.py <asset-name> <mpd-path> <output-path>")

    asset_name = sys.argv[1]
    if not asset_name:
        raise SystemExit("asset-name must not be empty")

    mpd_path = Path(sys.argv[2])
    output_path = Path(sys.argv[3])
    asset_dir = mpd_path.parent

    root = parse_mpd(mpd_path)
    validate_mpd_root(root)
    segment_template = find_segment_template(root)

    timescale = parse_optional_positive_int(segment_template.attrib, "timescale", 1)
    duration = parse_required_int(segment_template.attrib, "duration")
    start_number = parse_optional_positive_int(segment_template.attrib, "startNumber", 1)
    media_template = segment_template.attrib.get("media", "")
    init_segment = segment_template.attrib.get("initialization", "")

    init_segment_path = require_relative_asset_path(init_segment, "initialization")
    if not (asset_dir / init_segment_path).is_file():
        raise RuntimeError(f"initialization segment does not exist: {init_segment}")

    segment_duration_ms = round(duration * 1000 / timescale)
    if segment_duration_ms <= 0:
        raise RuntimeError(
            f"computed segment duration must be positive, got {segment_duration_ms}ms"
        )

    numbered_segments = collect_media_segments(asset_dir, media_template, start_number)
    sizes = [media_file.stat().st_size for _, media_file in numbered_segments]
    if any(size <= 0 for size in sizes):
        zero_length = [media_file.name for _, media_file in numbered_segments if media_file.stat().st_size <= 0]
        raise RuntimeError("generated zero-byte media segment(s): " + ", ".join(zero_length))

    lower_size_threshold = percentile(sizes, 0.25)
    upper_size_threshold = percentile(sizes, 0.75)

    segments = []
    for index, (sequence, media_file) in enumerate(numbered_segments):
        size_bytes = media_file.stat().st_size
        if index < 3:
            importance_hint = "critical"
            dependency_depth_hint = 0 if index == 0 else 1
            independent = index == 0
            priority_label = "startup"
        elif size_bytes >= upper_size_threshold:
            importance_hint = "high"
            dependency_depth_hint = 0
            independent = True
            priority_label = "burst"
        else:
            importance_hint = "normal"
            dependency_depth_hint = 1
            independent = False
            priority_label = "steady"

        if size_bytes <= lower_size_threshold:
            size_tier = "small"
            freshness_window_ms = segment_duration_ms
        elif size_bytes >= upper_size_threshold:
            size_tier = "large"
            freshness_window_ms = segment_duration_ms * 3
        else:
            size_tier = "medium"
            freshness_window_ms = segment_duration_ms * 2

        segments.append(
            {
                "sequence": sequence,
                "relative_path": media_file.name,
                "start_time_ms": index * segment_duration_ms,
                "duration_ms": segment_duration_ms,
                "size_bytes": size_bytes,
                "semantic_hint": {
                    "importance_hint": importance_hint,
                    "dependency_depth_hint": dependency_depth_hint,
                    "independent": independent,
                    "freshness_window_ms": freshness_window_ms,
                    "priority_label": priority_label,
                    "size_tier": size_tier,
                },
            }
        )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    replay_manifest = {
        "asset_name": asset_name,
        "init_segment": init_segment,
        "mpd_path": mpd_path.name,
        "segment_duration_ms": segment_duration_ms,
        "semantic_defaults": {
            "startup_segment_count": 3,
            "default_dependency_depth_hint": 1,
            "default_freshness_window_ms": segment_duration_ms * 2,
        },
        "segments": segments,
    }

    output_path.write_text(json.dumps(replay_manifest, indent=2), encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as exc:
        raise SystemExit(str(exc)) from exc