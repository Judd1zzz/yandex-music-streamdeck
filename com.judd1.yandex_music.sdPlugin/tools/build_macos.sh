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

echo "[1/2] Плагин universal2 (x86_64 + arm64)..."
( cd "$SRC" && cargo run -p xtask -- dist )

echo "[2/2] Упаковка релиза..."
( cd "$SRC" && cargo run -p xtask -- package )

echo
echo "Готово."
echo "  бинарь:  $(lipo -archs "$HERE/bin/ym-plugin" 2>/dev/null)"
echo "  релиз:   $HERE/../release/"
