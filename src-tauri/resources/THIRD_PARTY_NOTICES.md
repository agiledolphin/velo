# Third-party notices

Velo bundles separate media-processing executables and invokes them as child processes.

## yt-dlp

- Project: https://github.com/yt-dlp/yt-dlp
- Version: 2026.07.04
- License: The Unlicense

## FFmpeg

- Project and source: https://ffmpeg.org/
- macOS and Linux build source: https://ffmpeg.martin-riedl.de/
- Windows build source: https://www.gyan.dev/ffmpeg/builds/
- License information: https://ffmpeg.org/legal.html

The exact build asset and SHA-256 for each target are recorded in
`scripts/ffmpeg-manifest.ts`. FFmpeg is a separate executable and is not linked into
the Velo application. Review the applicable FFmpeg and third-party codec license
obligations before distributing a production release.
