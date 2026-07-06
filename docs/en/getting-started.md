# Getting Started

## Requirements

TMG1 Studio calls external command-line tools at runtime:

- **`ffmpeg` / `ffprobe`** — required. Put them on your `PATH`, or set their
  paths in the app settings.
- **[`tmg1`](https://github.com/tmg1-labs/tmg1-cli) CLI** — only needed for
  `.tmg1` export. Likewise on `PATH` or set in the app settings.

!!! important
    Frame width must be a multiple of 8 (`monob` byte boundary).

## Installation

Download the installer for your OS from the
[Releases page](https://github.com/tmg1-labs/tmg1-studio/releases).

![First launch](images/start.png)

## Building from source

- [Rust](https://rustup.rs/) and [Node.js](https://nodejs.org/) (18+)
- Tauri v2 [system dependencies](https://tauri.app/start/prerequisites/) for your OS

```bash
npm install
npm run tauri dev      # launch the app
```

## Configuring tool paths

If `ffmpeg`, `ffprobe`, or `tmg1` are not on your `PATH`, open the app
settings and point each entry at the executable.

![Settings](images/settings.png)
