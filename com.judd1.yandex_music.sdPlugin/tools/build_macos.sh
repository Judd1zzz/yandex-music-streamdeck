#!/usr/bin/env bash
set -euo pipefail

[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  echo "Ошибка: cargo не найден. Установи Rust (https://rustup.rs) или проверь ~/.cargo/bin." >&2
  exit 1
fi

HERE="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$HERE/src"
FFMPEG_BIN="$HERE/bin/ffmpeg"

if [ "${FORCE_FFMPEG:-0}" = "1" ] || ! lipo -archs "$FFMPEG_BIN" 2>/dev/null | grep -q x86_64; then
  echo "[1/3] Минимальный universal2 ffmpeg (долго при первой сборке)..."
  OUT_BIN="$FFMPEG_BIN" bash "$HERE/tools/build_ffmpeg_min.sh"
else
  echo "[1/3] ffmpeg уже universal2 — пропуск (FORCE_FFMPEG=1 чтобы пересобрать)"
fi
chmod +x "$FFMPEG_BIN"

echo "[2/3] Плагин universal2 (x86_64 + arm64)..."
( cd "$SRC" && cargo run -p xtask -- dist )

echo "[3/3] Упаковка релиза..."
( cd "$SRC" && cargo run -p xtask -- package )

echo
echo "Готово."
echo "  ffmpeg:  $(lipo -archs "$FFMPEG_BIN" 2>/dev/null)  ($(du -h "$FFMPEG_BIN" | cut -f1))"
echo "  бинарь:  $(lipo -archs "$HERE/bin/ym-plugin" 2>/dev/null)"
echo "  релиз:   $HERE/../release/"
