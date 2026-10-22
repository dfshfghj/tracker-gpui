$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $env:FFMPEG_DIR) {
    $env:FFMPEG_DIR = Join-Path $root "third_party\ffmpeg-n9.0-latest-win64-lgpl-shared-9.0"
}
if (-not (Test-Path (Join-Path $env:FFMPEG_DIR "include\libavcodec\avcodec.h"))) {
    throw "FFmpeg 9 shared build not found at $env:FFMPEG_DIR. Download it into third_party/ or set FFMPEG_DIR."
}

if (-not $env:LIBCLANG_PATH) {
    $candidates = @(
        (Join-Path $env:ProgramFiles "LLVM\bin"),
        "D:\Program Files\LLVM\bin",
        (Join-Path $env:USERPROFILE "scoop\apps\llvm\current\bin")
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path (Join-Path $candidate "libclang.dll"))) {
            $env:LIBCLANG_PATH = $candidate
            break
        }
    }
}
if (-not $env:LIBCLANG_PATH) {
    Write-Warning "LIBCLANG_PATH not set; install LLVM or set it to the folder containing libclang.dll."
}

$opencvBuild = Join-Path $root "third_party\opencv\opencv\build"
if (-not $env:OPENCV_LINK_PATHS -and (Test-Path (Join-Path $opencvBuild "x64\vc16\lib\opencv_world4120.lib"))) {
    $env:OPENCV_INCLUDE_PATHS = Join-Path $opencvBuild "include"
    $env:OPENCV_LINK_PATHS = Join-Path $opencvBuild "x64\vc16\lib"
    $env:OPENCV_LINK_LIBS = "opencv_world4120"
}

Write-Host "FFMPEG_DIR=$env:FFMPEG_DIR"
if ($env:OPENCV_LINK_LIBS) { Write-Host "OPENCV_LINK_LIBS=$env:OPENCV_LINK_LIBS" }
if ($env:LIBCLANG_PATH) { Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH" }

$cargoArgs = @($args)
$featureCommands = @("build", "run", "test", "check", "clippy")
$hasFeatures = $false
foreach ($arg in $cargoArgs) {
    if ($arg -eq "--features" -or $arg -like "--features=*" -or $arg -eq "--no-default-features") {
        $hasFeatures = $true
    }
}
$autoVision = $env:OPENCV_LINK_LIBS -and $cargoArgs.Count -gt 0 -and
    ($featureCommands -contains $cargoArgs[0]) -and -not $hasFeatures
if ($autoVision) {
    $cargoArgs += @("--features", "vision")
}

if ($cargoArgs.Count -eq 0) {
    if ($env:OPENCV_LINK_LIBS) {
        & cargo build --features vision
    } else {
        & cargo build
    }
} else {
    & cargo @cargoArgs
}
exit $LASTEXITCODE
