# TMG1 Studio

A cross-platform desktop GUI for converting video to 1-bit monochrome **per segment**,
built for the [TMG1](https://github.com/tmg1-labs) pipeline (ESP32 OLED playback).

A single uniform monochrome setting either loses detail or adds noise. TMG1 Studio lets
you split the timeline into segments and tune contrast / level-squeeze / dithering
independently for each, previewing the exact `monob` (1-bit) result as you go.

日本語版は [README.ja.md](README.ja.md) を参照。

## Features

- Load a video and scrub the timeline; preview the **1-bit `monob` output** at any time.
- Split the timeline into segments; drag boundaries; merge segments.
- Per-segment parameters:
  - **Contrast** (`eq=contrast`)
  - **Level squeeze** — crush shadows below a low threshold, blow out highlights above a
    high threshold (removes stray white dots in dark areas / dropouts in the foreground).
  - **Dither** — Bayer / error-diffusion / none (`-sws_dither`).
- Export splits each segment, transcodes with its own settings, and concatenates the raw
  losslessly. Choose the output format — `raw`, `tmg1`, or both — and the app invokes the
  `tmg1` CLI itself to produce a ready-to-play `.tmg1` (no separate encode step). A
  nearest-neighbour upscaled `.preview.mp4` for eyeballing is an optional extra (off by default).

The **same filter-chain builder** (`src-tauri/src/filter.rs`) is used for both preview and
export, so what you see is what you get.

## Architecture

```
tmg1-studio/
├── src-tauri/            ... Rust backend (Tauri v2)
│   ├── src/filter.rs         ... filter-chain builder (single source of truth)
│   ├── src/ffmpeg.rs         ... ffmpeg / ffprobe process wrappers
│   └── src/lib.rs            ... Tauri commands (probe_video / render_preview / export)
└── src/                  ... Web frontend (Vanilla TS): timeline / params / preview
```

- **Preview fidelity**: `monob` only (no TMG1 round-trip). TMG1 encoding is lossless, so the
  monochrome preview matches the on-device pixels.
- **External tools**: uses the system `ffmpeg` / `ffprobe` (and `tmg1` for `tmg1` export) —
  none are bundled. Each executable is found on `PATH` by default, or you can point to a
  specific path in the app settings.

## Prerequisites

- [Rust](https://rustup.rs/) + [Node.js](https://nodejs.org/) (18+)
- `ffmpeg` and `ffprobe` available on `PATH` (or set their paths in the app settings)
- For `tmg1` export: the [`tmg1`](https://github.com/tmg1-labs/tmg1-cli) CLI on `PATH` (or set its path in the app settings)
- Tauri v2 [system dependencies](https://tauri.app/start/prerequisites/) for your OS

## Development

```bash
npm install
npm run tauri dev      # launch the app
```

Backend unit tests (filter-chain builder):

```bash
cd src-tauri && cargo test
```

## Releases

Pushing a `v*` tag (e.g. `v0.2.0`) triggers `.github/workflows/release.yml`, which builds
installers on native runners for Windows (x64), macOS (Apple Silicon), and Linux (x64) and
attaches them to a **draft** GitHub Release. The tag's version is synced into `package.json`,
`tauri.conf.json`, and `Cargo.toml` at build time (`scripts/sync-version.mjs`), and passed to
the app as the displayed version via `VITE_APP_VERSION`. Review the draft, then publish it.

> Installers are **not code-signed**, so macOS Gatekeeper and Windows SmartScreen will warn on
> first launch (right-click → Open on macOS; "More info → Run anyway" on Windows).

Push/PR checks (`tsc` + `cargo test` + `clippy`) run separately in `.github/workflows/ci.yml`.

## Export output

Depending on the chosen format:

- `<name>.tmg1` — ready-to-play stream, encoded by invoking the `tmg1` CLI (format `tmg1` or `both`).
- `<name>.raw` — packed `monob` frames, for feeding to `tmg1-cli encode` yourself (format `raw` or `both`).
- `<name>.preview.mp4` — 6× nearest-neighbour upscale for visual confirmation (optional, off by default).

> Width must be a multiple of 8 (monob byte boundary).

## Related

- [`tmg1-codec`](https://github.com/tmg1-labs/tmg1-codec) — shared C++ codec library
- [`tmg1-cli`](https://github.com/tmg1-labs/tmg1-cli) — Rust CLI (encode/decode/transcode)
- [`tmg1-esp32-demo`](https://github.com/tmg1-labs/tmg1-esp32-demo) — ESP32 OLED reference player

## License

MIT
