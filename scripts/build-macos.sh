#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -n "${FFMPEG_DIR:-}" ]]; then
  if [[ ! -f "$FFMPEG_DIR/include/libavcodec/avcodec.h" ]]; then
    echo "FFMPEG_DIR is set but does not look like an FFmpeg prefix: $FFMPEG_DIR" >&2
    exit 1
  fi
else
  if ! command -v pkg-config >/dev/null 2>&1; then
    echo "pkg-config not found. Install it with: brew install pkg-config" >&2
    exit 1
  fi
  ffmpeg_prefix=""
  if command -v brew >/dev/null 2>&1; then
    brew_prefix="$(brew --prefix)"
    ffmpeg_prefix="$(brew --prefix ffmpeg 2>/dev/null || true)"
    export PKG_CONFIG_PATH="$brew_prefix/lib/pkgconfig:${ffmpeg_prefix:+$ffmpeg_prefix/lib/pkgconfig:}${PKG_CONFIG_PATH:-}"
    if [[ -n "$ffmpeg_prefix" ]]; then
      export DYLD_FALLBACK_LIBRARY_PATH="$ffmpeg_prefix/lib:${DYLD_FALLBACK_LIBRARY_PATH:-/usr/local/lib:/usr/lib}"
    fi
  fi
  if ! pkg-config --exists libavcodec; then
    echo "FFmpeg development files not found. Install with: brew install ffmpeg" >&2
    echo "The ffmpeg-the-third dependency expects a shared FFmpeg 9 build." >&2
    exit 1
  fi
  avcodec_version="$(pkg-config --modversion libavcodec)"
  echo "libavcodec: $avcodec_version"
  avcodec_major="${avcodec_version%%.*}"
  if [[ "$avcodec_major" =~ ^[0-9]+$ ]] && (( avcodec_major < 63 )); then
    echo "warning: expected FFmpeg 9 (libavcodec major 63), found $avcodec_version." >&2
    echo "         Install FFmpeg 9 or set FFMPEG_DIR to an FFmpeg 9 prefix." >&2
  fi
fi

if [[ -n "${FFMPEG_DIR:-}" ]]; then
  export DYLD_FALLBACK_LIBRARY_PATH="$FFMPEG_DIR/lib:${DYLD_FALLBACK_LIBRARY_PATH:-/usr/local/lib:/usr/lib}"
fi

if [[ -z "${LIBCLANG_PATH:-}" ]] && command -v brew >/dev/null 2>&1; then
  llvm_prefix="$(brew --prefix llvm 2>/dev/null || true)"
  if [[ -n "$llvm_prefix" && -e "$llvm_prefix/lib/libclang.dylib" ]]; then
    export LIBCLANG_PATH="$llvm_prefix/lib"
  fi
fi
if [[ -n "${LIBCLANG_PATH:-}" ]]; then
  echo "LIBCLANG_PATH=$LIBCLANG_PATH"
fi

if [[ $# -eq 0 ]]; then
  set -- build
fi
exec cargo "$@"
