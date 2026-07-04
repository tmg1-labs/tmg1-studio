# TMG1 Studio

動画を**区間ごとに**1bit モノクロ化するクロスプラットフォームなデスクトップ GUI。
[TMG1](https://github.com/tmg1-labs) パイプライン（ESP32 OLED 再生）向け。

全体を一律設定でモノクロ化すると、ディテール不足かノイズ増加のどちらかに寄る。
TMG1 Studio はタイムラインを区間に分割し、コントラスト / レベル絞り / ディザを区間ごとに
調整できる。1bit `monob` の出力そのものをプレビューしながら追い込める。

English: [README.md](README.md)

## 機能

- 動画を読み込みタイムラインをスクラブ。任意時刻の **1bit `monob` 出力**をプレビュー。
- タイムラインを区間に分割 / 境界をドラッグ / 区間を結合削除。
- 区間ごとのパラメータ:
  - **コントラスト**（`eq=contrast`）
  - **レベル絞り** — 下限未満を黒潰し、上限超を白飛ばし（暗部の孤立白点・前景の欠け対策）。
  - **ディザ** — Bayer / 誤差拡散 / なし（`-sws_dither`）。
- エクスポートは各区間を個別設定でトランスコードし raw を無劣化連結。あわせて目視確認用に
  近傍拡大した `.preview.mp4` も出力。

プレビューとエクスポートは**同一のフィルタチェーン組み立て**（`src-tauri/src/filter.rs`）を
使うため、見た目と出力が一致する。

## アーキテクチャ

```
tmg1-studio/
├── src-tauri/            ... Rust バックエンド (Tauri v2)
│   ├── src/filter.rs         ... フィルタチェーン組み立て（単一の真実の源）
│   ├── src/ffmpeg.rs         ... ffmpeg / ffprobe プロセスラッパ
│   └── src/lib.rs            ... Tauri コマンド (probe_video / render_preview / export)
└── src/                  ... Web フロントエンド (Vanilla TS): タイムライン / パラメータ / プレビュー
```

- **プレビュー忠実度**: `monob` のみ（TMG1 ラウンドトリップはしない）。TMG1 エンコードは
  ロスレスなので、モノクロプレビューが実機ピクセルと一致する。
- **ffmpeg**: システム PATH の `ffmpeg` / `ffprobe` を使う（同梱しない）。

## 前提

- [Rust](https://rustup.rs/) + [Node.js](https://nodejs.org/)（18 以上）
- PATH に `ffmpeg` / `ffprobe`
- OS ごとの Tauri v2 [システム依存](https://tauri.app/start/prerequisites/)

## 開発

```bash
npm install
npm run tauri dev      # アプリを起動
```

バックエンドの単体テスト（フィルタチェーン組み立て）:

```bash
cd src-tauri && cargo test
```

## エクスポート出力

- `<name>.raw` — パックされた `monob` フレーム（`tmg1-cli encode` に渡して `.tmg1` 化）。
- `<name>.preview.mp4` — 6 倍近傍拡大の目視確認用。

> 幅は 8 の倍数にすること（monob のバイト境界）。

## 関連リポジトリ

- [`tmg1-codec`](https://github.com/tmg1-labs/tmg1-codec) — 共通 C++ コーデックライブラリ
- [`tmg1-cli`](https://github.com/tmg1-labs/tmg1-cli) — Rust CLI（encode/decode/transcode）
- [`tmg1-esp32-demo`](https://github.com/tmg1-labs/tmg1-esp32-demo) — ESP32 OLED リファレンス再生

## ライセンス

MIT
