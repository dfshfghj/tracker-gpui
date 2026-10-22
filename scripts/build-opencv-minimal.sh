#!/usr/bin/env bash
# Build the small OpenCV subset required by tracker-gpui's vision feature.
set -euo pipefail

VERSION="${OPENCV_VERSION:-4.12.0}"
PREFIX="${1:-${OPENCV_PREFIX:-$PWD/opencv-prefix}}"
ROOT="${OPENCV_SOURCE_DIR:-$PWD/opencv-src}"
ARCHIVE="$ROOT/opencv-$VERSION.tar.gz"
SOURCE="$ROOT/opencv-$VERSION"
BUILD="$ROOT/build-$VERSION"

if [ -f "$PREFIX/include/opencv2/opencv.hpp" ] &&
   ls "$PREFIX"/lib/libopencv_world* >/dev/null 2>&1; then
    echo "OpenCV $VERSION already installed at $PREFIX"
    exit 0
fi

mkdir -p "$ROOT"
if [ ! -d "$SOURCE" ]; then
    echo "Downloading OpenCV $VERSION"
    curl -fsSL -o "$ARCHIVE" \
        "https://github.com/opencv/opencv/archive/refs/tags/$VERSION.tar.gz"
    tar -xzf "$ARCHIVE" -C "$ROOT"
fi

JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu)"
cmake -S "$SOURCE" -B "$BUILD" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DCMAKE_INSTALL_NAME_DIR=@rpath \
    -DCMAKE_INSTALL_RPATH=@loader_path \
    -DBUILD_LIST=core,imgproc,imgcodecs,video,objdetect,calib3d,features2d,flann \
    -DBUILD_opencv_world=ON \
    -DBUILD_SHARED_LIBS=ON \
    -DBUILD_TESTS=OFF \
    -DBUILD_PERF_TESTS=OFF \
    -DBUILD_EXAMPLES=OFF \
    -DBUILD_DOCS=OFF \
    -DBUILD_opencv_apps=OFF \
    -DBUILD_opencv_python3=OFF \
    -DBUILD_JAVA=OFF \
    -DBUILD_opencv_java=OFF \
    -DBUILD_opencv_js=OFF \
    -DWITH_OPENCL=OFF \
    -DWITH_IPP=OFF \
    -DWITH_TBB=OFF \
    -DWITH_OPENMP=OFF \
    -DWITH_FFMPEG=OFF \
    -DWITH_QT=OFF \
    -DWITH_GTK=OFF \
    -DWITH_V4L=OFF \
    -DWITH_ADE=OFF \
    -DWITH_EIGEN=OFF \
    -DWITH_LAPACK=OFF \
    -DWITH_JPEG=ON \
    -DWITH_PNG=ON \
    -DWITH_TIFF=OFF \
    -DWITH_OPENEXR=OFF \
    -DWITH_WEBP=OFF \
    -DWITH_OPENJPEG=OFF \
    -DWITH_JASPER=OFF \
    -DWITH_PROTOBUF=OFF \
    -DWITH_QUIRC=OFF \
    -DOPENCV_ENABLE_NONFREE=OFF \
    -DOPENCV_GENERATE_PKGCONFIG=OFF
cmake --build "$BUILD" --config Release --target install --parallel "$JOBS"

mkdir -p "$PREFIX/licenses/opencv"
cp "$SOURCE/LICENSE" "$PREFIX/licenses/opencv/LICENSE"
echo "OpenCV $VERSION installed at $PREFIX"
