#!/usr/bin/env bash

set -euo pipefail
shopt -s nullglob

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
raw_dir="${root_dir}/data/raw"
segments_dir="${root_dir}/data/processed/segments"
manifests_dir="${root_dir}/data/processed/manifests"
replay_manifest_builder="${root_dir}/scripts/media/build_replay_manifest.py"

mkdir -p "${segments_dir}" "${manifests_dir}"

require_command() {
    local command_name="$1"

    if ! command -v "${command_name}" >/dev/null 2>&1; then
        echo "missing required command: ${command_name}" >&2
        exit 1
    fi
}

require_file() {
    local file_path="$1"
    local description="$2"

    if [[ ! -f "${file_path}" ]]; then
        echo "missing ${description}: ${file_path}" >&2
        exit 1
    fi

    if [[ ! -s "${file_path}" ]]; then
        echo "${description} is empty: ${file_path}" >&2
        exit 1
    fi
}

validate_generated_asset() {
    local asset_name="$1"
    local asset_segments_dir="$2"
    local manifest_path="${asset_segments_dir}/manifest.mpd"
    local init_path="${asset_segments_dir}/${asset_name}_init.mp4"
    local segment_files=("${asset_segments_dir}"/*.m4s)

    require_file "${manifest_path}" "generated MPD"
    require_file "${init_path}" "generated init segment"

    if [[ ${#segment_files[@]} -eq 0 ]]; then
        echo "no media segments generated for ${asset_name} under ${asset_segments_dir}" >&2
        exit 1
    fi

    local segment_file
    for segment_file in "${segment_files[@]}"; do
        if [[ ! -s "${segment_file}" ]]; then
            echo "generated media segment is empty: ${segment_file}" >&2
            exit 1
        fi
    done
}

require_command ffmpeg
require_command ffprobe
require_command python
require_file "${replay_manifest_builder}" "replay manifest builder"

pick_video_encoder() {
    local encoders

    encoders="$(ffmpeg -hide_banner -encoders)"

    if grep -q " h264_mf " <<< "${encoders}"; then
        echo "h264_mf"
        return
    fi

    if grep -q " mpeg4 " <<< "${encoders}"; then
        echo "mpeg4"
        return
    fi

    echo "no supported encoder found for preprocessing" >&2
    exit 1
}

video_encoder="$(pick_video_encoder)"

preprocess_asset() {
    local input_path="$1"
    local asset_name="$2"
    local asset_segments_dir="${segments_dir}/${asset_name}"

    require_file "${input_path}" "input media file"

    mkdir -p "${asset_segments_dir}"
    rm -f "${asset_segments_dir}"/*

    ffmpeg -y \
        -i "${input_path}" \
        -an \
        -c:v "${video_encoder}" \
        -g 30 \
        -keyint_min 30 \
        -sc_threshold 0 \
        -f dash \
        -seg_duration 1 \
        -use_template 1 \
        -use_timeline 0 \
        -streaming 1 \
        -ldash 0 \
        -init_seg_name "${asset_name}_init.mp4" \
        -media_seg_name "${asset_name}_chunk_\$Number%05d\$.m4s" \
        "${asset_segments_dir}/manifest.mpd"

    validate_generated_asset "${asset_name}" "${asset_segments_dir}"

    ffprobe \
        -v error \
        -select_streams v:0 \
        -show_frames \
        -show_entries frame=best_effort_timestamp_time,pkt_duration_time,pkt_size,key_frame,pict_type \
        -of json \
        "${input_path}" > "${manifests_dir}/${asset_name}_frames.json"

    ffprobe \
        -v error \
        -show_format \
        -show_streams \
        -of json \
        "${input_path}" > "${manifests_dir}/${asset_name}_container.json"

    python "${replay_manifest_builder}" \
        "${asset_name}" \
        "${asset_segments_dir}/manifest.mpd" \
        "${manifests_dir}/${asset_name}_replay.json"

    require_file "${manifests_dir}/${asset_name}_frames.json" "frame manifest"
    require_file "${manifests_dir}/${asset_name}_container.json" "container manifest"
    require_file "${manifests_dir}/${asset_name}_replay.json" "replay manifest"
}

preprocess_asset "${raw_dir}/big_buck_bunny_320x180.mp4" "big_buck_bunny"
preprocess_asset "${raw_dir}/sintel_trailer_480p.mp4" "sintel_trailer"

echo "done: processed CMAF-style segments under ${segments_dir} and manifests under ${manifests_dir} using ${video_encoder}"