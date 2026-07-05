# TMG1 Studio

**English** | [日本語](README.ja.md)

![TMG1 Studio screenshot](docs/screenshot.webp)

A cross-platform desktop GUI for turning video into 1-bit monochrome (`monob`)
footage, tuned **per segment** of the timeline. Despite the name, its main job is
authoring the packed `monob` raw file — direct `.tmg1` export is a convenience
layered on top.

A single uniform monochrome setting either loses detail or adds noise, so TMG1
Studio lets you split the timeline into segments and tune contrast / level-squeeze /
dithering independently for each, previewing the exact 1-bit `monob` result as you
go. That raw can then be encoded to [TMG1](https://github.com/tmg1-labs) and played
back on an ESP32-driven OLED — and TMG1 Studio can run that `tmg1` encode for you,
emitting a `.tmg1` directly.

## Features

- Load a video and scrub the timeline; preview the **1-bit `monob` output** at any time.
- Split the timeline into segments; drag boundaries; merge segments.
- Per-segment parameters:
  - **Contrast** (`eq=contrast`)
  - **Level squeeze** — crush shadows below a low threshold, blow out highlights above a
    high threshold (removes stray white dots in dark areas / dropouts in the foreground).
  - **Dither** — Bayer / error-diffusion / none (`-sws_dither`).
- Export splits each segment, transcodes with its own settings, and concatenates the raw
  losslessly. Choose the output format — `raw`, `tmg1`, or both — and for `tmg1` the app
  invokes the `tmg1` CLI itself to encode that raw into `.tmg1` (no separate encode step). A
  nearest-neighbour upscaled `.preview.mp4` for eyeballing is an optional extra (off by default).

The **same filter-chain builder** (`src-tauri/src/filter.rs`) is used for both preview and
export, so what you see is what you get.

## Export output

Export renders every segment with its own settings and concatenates them into one
monochrome result for the whole timeline. You pick the primary output format:

- **`raw`** — writes `<name>.raw`: packed 1-bit `monob` frames. Encode them to TMG1
  whenever you like with `tmg1-cli encode`.
- **`tmg1`** — writes `<name>.tmg1`: that raw encoded to the TMG1 format. Studio runs
  the `tmg1` CLI for you, so there is no separate encode step.
- **`both`** — writes both of the above.

You can optionally also write `<name>.preview.mp4` — a 6× nearest-neighbour upscale
for eyeballing the result on a normal display (off by default).

> [!IMPORTANT]
> The frame width must be a multiple of 8 (the `monob` byte boundary).

## Prerequisites

**To build (running from source)**

- [Rust](https://rustup.rs/) + [Node.js](https://nodejs.org/) (18+)
- Tauri v2 [system dependencies](https://tauri.app/start/prerequisites/) for your OS

**To run (using the app)**

- `ffmpeg` and `ffprobe` — on `PATH`, or set their paths in the app settings
- For `tmg1` export only: the [`tmg1`](https://github.com/tmg1-labs/tmg1-cli) CLI — likewise on `PATH` or set in the app settings

## Development

```bash
npm install
npm run tauri dev      # launch the app
```

Backend unit tests (filter-chain builder):

```bash
cd src-tauri && cargo test
```

Push/PR checks (`tsc` + `cargo test` + `clippy`) run in `.github/workflows/ci.yml`.

## Related projects

Part of **[TMG1 Labs](https://github.com/tmg1-labs)** — see the organization
profile for all repositories in the project.

## License

MIT
