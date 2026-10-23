# Video export fixtures

Synthetic black frames (96 × 64), generated locally with FFmpeg 9; no third-party media.

- export-source.mp4: 12 MPEG-4 frames at 12 fps and a 440 Hz AAC tone, one second.
- export-vfr.mp4: 12 MPEG-4 frames; first six at 12 fps, remaining frames at 6 fps.

Tests decode and export these through libavcodec/libavformat directly, including in CI's trimmed FFmpeg build. No ffmpeg executable or AAC encoder/decoder is required to run the tests; audio is packet copied.
