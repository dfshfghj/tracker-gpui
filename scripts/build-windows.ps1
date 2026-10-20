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

Write-Host "FFMPEG_DIR=$env:FFMPEG_DIR"
if ($env:LIBCLANG_PATH) { Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH" }
if ($args.Count -eq 0) {
    & cargo build
} else {
    & cargo @args
}
exit $LASTEXITCODE
