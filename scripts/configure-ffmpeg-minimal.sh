#!/usr/bin/env bash
set -euo pipefail

# Run from an FFmpeg source tree. This keeps only the demuxers/decoders the
# Tracker video layer can actually use, plus native MPEG-4 / MP4 export.
# Audio can be stream-copied; no audio decoder or encoder is required.
#
# Usage:
#   ../scripts/configure-ffmpeg-minimal.sh --prefix="$PWD/../prefix" [extra args]

./configure \
  --disable-everything \
  --disable-autodetect \
  --disable-network \
  --disable-doc \
  --disable-programs \
  --disable-avdevice \
  --disable-avfilter \
  --disable-encoders \
  --disable-muxers \
  --disable-devices \
  --disable-filters \
  --disable-hwaccels \
  --disable-iconv \
  --disable-zlib \
  --disable-bzlib \
  --disable-lzma \
  --disable-sdl2 \
  --disable-debug \
  --enable-shared \
  --disable-static \
  --enable-pic \
  --disable-gpl \
  --disable-version3 \
  --disable-nonfree \
  --enable-avutil \
  --enable-avcodec \
  --enable-avformat \
  --enable-swscale \
  --enable-encoder=mpeg4 \
  --enable-muxer=mp4 \
  --enable-protocol=file \
  --enable-demuxer=mov,matroska,avi,flv,asf,mpegps,mpegts,mpegvideo,ogg \
  --enable-decoder=h264,hevc,mpeg4,mpeg1video,mpeg2video,mjpeg,mjpegb,prores,theora,vp8,vp9,av1,flv,h263,h263p,wmv1,wmv2,wmv3,vc1,msmpeg4v1,msmpeg4v2,msmpeg4v3,dvvideo,ffv1,huffyuv,utvideo,dnxhd,rawvideo \
  --enable-parser=h264,hevc,mpeg4video,mpegvideo,mjpeg,vp3,vp8,vp9,vc1,h263 \
  --enable-bsf=h264_mp4toannexb,hevc_mp4toannexb,mpeg4_unpack_bframes,aac_adtstoasc \
  "$@"
