#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$HERE/bin"

OUT_BIN="$HERE/bin/ffmpeg.exe" TARGET=windows bash "$HERE/tools/build_ffmpeg_min.sh"
