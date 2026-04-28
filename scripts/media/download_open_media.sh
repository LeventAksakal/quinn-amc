#!/usr/bin/env bash

set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
raw_dir="${root_dir}/data/raw"

mkdir -p "${raw_dir}"

download() {
    local url="$1"
    local output="$2"

    if [[ -f "${output}" ]]; then
        echo "skip: ${output} already exists"
        return
    fi

    echo "download: ${url}"
    curl -L --fail --retry 3 --output "${output}" "${url}"
}

download "https://download.blender.org/peach/bigbuckbunny_movies/BigBuckBunny_320x180.mp4" "${raw_dir}/big_buck_bunny_320x180.mp4"
download "https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4" "${raw_dir}/sintel_trailer_480p.mp4"

echo "done: source clips are available under ${raw_dir}"