param(
    [string]$Prefix = "$PWD/opencv-prefix",
    [string]$Version = "4.12.0"
)

$ErrorActionPreference = "Stop"
$root = Join-Path $PWD "opencv-src"
$archive = Join-Path $root "opencv-$Version.zip"
$source = Join-Path $root "opencv-$Version"
$build = Join-Path $root "build-$Version"

$installed = (Test-Path (Join-Path $Prefix "include\opencv2\opencv.hpp")) -and
    (Get-ChildItem -Path $Prefix -Recurse -Filter "opencv_world*.lib" -ErrorAction SilentlyContinue)
if ($installed) {
    Write-Host "OpenCV $Version already installed at $Prefix"
    exit 0
}

New-Item -ItemType Directory -Force -Path $root | Out-Null
if (!(Test-Path $source)) {
    Write-Host "Downloading OpenCV $Version"
    Invoke-WebRequest -Uri "https://github.com/opencv/opencv/archive/refs/tags/$Version.zip" -OutFile $archive
    Expand-Archive -Path $archive -DestinationPath $root -Force
}

cmake -S $source -B $build -G Ninja `
    -DCMAKE_BUILD_TYPE=Release `
    -DCMAKE_INSTALL_PREFIX="$Prefix" `
    "-DBUILD_LIST=core,imgproc,imgcodecs,video,objdetect,calib3d,features2d,flann" `
    -DBUILD_opencv_world=ON `
    -DBUILD_SHARED_LIBS=ON `
    -DBUILD_TESTS=OFF `
    -DBUILD_PERF_TESTS=OFF `
    -DBUILD_EXAMPLES=OFF `
    -DBUILD_DOCS=OFF `
    -DBUILD_opencv_apps=OFF `
    -DBUILD_opencv_highgui=OFF `
    -DBUILD_opencv_python3=OFF `
    -DBUILD_JAVA=OFF `
    -DBUILD_opencv_java=OFF `
    -DBUILD_opencv_js=OFF `
    -DWITH_OPENCL=OFF `
    -DWITH_IPP=OFF `
    -DWITH_TBB=OFF `
    -DWITH_OPENMP=OFF `
    -DWITH_FFMPEG=OFF `
    -DWITH_QT=OFF `
    -DWITH_GTK=OFF `
    -DWITH_V4L=OFF `
    -DWITH_ADE=OFF `
    -DWITH_EIGEN=OFF `
    -DWITH_LAPACK=OFF `
    -DWITH_JPEG=ON `
    -DWITH_PNG=ON `
    -DWITH_TIFF=OFF `
    -DWITH_OPENEXR=OFF `
    -DWITH_WEBP=OFF `
    -DWITH_OPENJPEG=OFF `
    -DWITH_JASPER=OFF `
    -DWITH_PROTOBUF=OFF `
    -DWITH_QUIRC=OFF `
    -DOPENCV_ENABLE_NONFREE=OFF `
    -DOPENCV_GENERATE_PKGCONFIG=OFF
if ($LASTEXITCODE -ne 0) { throw "OpenCV configure failed" }

cmake --build $build --target install --parallel
if ($LASTEXITCODE -ne 0) { throw "OpenCV build failed" }

$licenseDir = Join-Path $Prefix "licenses\opencv"
New-Item -ItemType Directory -Force -Path $licenseDir | Out-Null
Copy-Item (Join-Path $source "LICENSE") (Join-Path $licenseDir "LICENSE")
Write-Host "OpenCV $Version installed at $Prefix"
