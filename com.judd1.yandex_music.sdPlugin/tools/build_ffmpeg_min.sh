#!/usr/bin/env bash
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-7.1}"
LAME_VERSION="${LAME_VERSION:-3.100}"
WORK="${WORK:-/tmp/ym_ffmpeg_build}"
OUT_BIN="${OUT_BIN:-$WORK/ffmpeg-min}"
TARGET="${TARGET:-darwin}"

FF_COMMON=(
  --disable-everything --disable-x86asm
  --disable-doc --disable-htmlpages --disable-manpages --disable-podpages --disable-txtpages
  --disable-ffplay --disable-ffprobe --disable-network --disable-autodetect
  --enable-small --enable-libmp3lame --enable-protocol=file
  --enable-demuxer=mov,flac,aac,mp3,ogg,matroska,wav
  --enable-decoder=flac,alac,aac,mp3,vorbis,opus,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s32le
  --enable-encoder=libmp3lame,flac,alac
  --enable-muxer=flac,ipod,mp3,mp4
  --enable-parser=flac,aac,mpegaudio,vorbis,opus
  --enable-filter=aresample,aformat,anull
)

mkdir -p "$WORK"
cd "$WORK"

fetch() {
  local url="$1" out="$2"
  [ -f "$out" ] || curl -fsSL "$url" -o "$out"
}

fetch "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" ffmpeg.tar.xz
fetch "https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz" lame.tar.gz

refresh_config_sub() {
  local dir="$1"
  for cf in config.sub config.guess; do
    fetch "https://git.savannah.gnu.org/cgit/config.git/plain/$cf" "$WORK/$cf"
    cp "$WORK/$cf" "$dir/$cf"
    chmod +x "$dir/$cf"
  done
}

build_arch() {
  local arch="$1"
  local lame_prefix="$WORK/lame-$arch"
  local ff_src="$WORK/ffmpeg-$arch"
  local cc="clang -arch $arch"

  rm -rf "$WORK/lame-src-$arch" "$ff_src"
  tar xf lame.tar.gz && mv "lame-$LAME_VERSION" "$WORK/lame-src-$arch"
  refresh_config_sub "$WORK/lame-src-$arch"
  ( cd "$WORK/lame-src-$arch" && \
    ./configure --host="$arch-apple-darwin" --prefix="$lame_prefix" \
      CC="$cc" --disable-shared --enable-static --disable-frontend >/dev/null && \
    make -j"$(sysctl -n hw.ncpu)" >/dev/null && make install >/dev/null )

  tar xf ffmpeg.tar.xz && mv "ffmpeg-$FFMPEG_VERSION" "$ff_src"
  ( cd "$ff_src" && \
    ./configure \
      --cc="$cc" --arch="$arch" --enable-cross-compile --target-os=darwin \
      "${FF_COMMON[@]}" \
      --extra-cflags="-I$lame_prefix/include" \
      --extra-ldflags="-L$lame_prefix/lib" \
      --extra-libs="-lmp3lame -lm" >/dev/null && \
    make -j"$(sysctl -n hw.ncpu)" >/dev/null )
  cp "$ff_src/ffmpeg" "$WORK/ffmpeg-bin-$arch"
}

build_win_exe() {
  local arch=x86_64
  local host=x86_64-w64-mingw32
  local cross="$(brew --prefix)/bin/${host}-"
  if ! command -v "${cross}gcc" >/dev/null 2>&1; then
    echo "mingw-w64 не найден (${cross}gcc). Установите: brew install mingw-w64" >&2
    exit 1
  fi
  local lame_prefix="$WORK/lame-win"
  local ff_src="$WORK/ffmpeg-win"

  rm -rf "$WORK/lame-src-win" "$ff_src"
  tar xf lame.tar.gz && mv "lame-$LAME_VERSION" "$WORK/lame-src-win"
  refresh_config_sub "$WORK/lame-src-win"
  ( cd "$WORK/lame-src-win" && \
    ./configure --host="$host" --prefix="$lame_prefix" \
      CC="${cross}gcc" --disable-shared --enable-static --disable-frontend >/dev/null && \
    make -j"$(sysctl -n hw.ncpu)" >/dev/null && make install >/dev/null )

  tar xf ffmpeg.tar.xz && mv "ffmpeg-$FFMPEG_VERSION" "$ff_src"
  ( cd "$ff_src" && \
    ./configure \
      --cc="${cross}gcc" --arch="$arch" --target-os=mingw32 --cross-prefix="$cross" --enable-cross-compile \
      "${FF_COMMON[@]}" \
      --extra-cflags="-I$lame_prefix/include" \
      --extra-ldflags="-L$lame_prefix/lib -static" \
      --extra-libs="-lmp3lame -lm" >/dev/null && \
    make -j"$(sysctl -n hw.ncpu)" >/dev/null )
  cp "$ff_src/ffmpeg.exe" "$OUT_BIN"
  "${cross}strip" "$OUT_BIN" 2>/dev/null || true
}

if [ "$TARGET" = "windows" ]; then
  build_win_exe
  chmod +x "$OUT_BIN" 2>/dev/null || true
  echo "=== minimal ffmpeg.exe (mingw x86_64) ==="
  file "$OUT_BIN" 2>/dev/null || true
  ls -lh "$OUT_BIN"
  echo "-> $OUT_BIN"
  exit 0
fi

if [ "$(uname -s)" = "Darwin" ]; then
  build_arch arm64
  build_arch x86_64
  lipo -create -output "$OUT_BIN" "$WORK/ffmpeg-bin-arm64" "$WORK/ffmpeg-bin-x86_64"
else
  build_arch "$(uname -m)"
  cp "$WORK/ffmpeg-bin-$(uname -m)" "$OUT_BIN"
fi

strip "$OUT_BIN" 2>/dev/null || true
chmod +x "$OUT_BIN"
echo "=== minimal ffmpeg ==="
lipo -archs "$OUT_BIN" 2>/dev/null || file "$OUT_BIN"
ls -lh "$OUT_BIN"
echo "-> $OUT_BIN"
