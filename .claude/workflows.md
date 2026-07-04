# よく使うコマンド・手順

## 前提
- [Rust](https://rustup.rs/)（rustup）+ [Node.js](https://nodejs.org/)（18+）
- `ffmpeg` / `ffprobe` が `PATH` にあること（同梱しない）
- OS ごとの Tauri v2 システム依存（https://tauri.app/start/prerequisites/）

## セットアップ
```bash
npm install
```

## 開発サーバー起動（アプリを立ち上げる）
```bash
npm run tauri dev
```
- Vite は固定ポート 1420 を使う（`strictPort`。空いていないと失敗する）。

## テスト
```bash
# バックエンド単体テスト（フィルタチェーンビルダ）
cd src-tauri && cargo test
```

## ビルド
```bash
# フロント型チェック + Vite ビルド
npm run build

# 配布用ネイティブビルド（Tauri バンドル）
npm run tauri build
```

## エクスポート出力
- `<name>.raw` — packed `monob` フレーム。`tmg1-cli encode` に渡して `.tmg1` を生成する。
- `<name>.preview.mp4` — 6× 最近傍アップスケール（目視確認用）。
- 幅は 8 の倍数であること（monob バイト境界）。

## パイプライン上の位置
動画 → **[TMG1 Studio]** 区間ごと monob 調整 → `.raw` → `tmg1-cli encode` → `.tmg1`
→ `tmg1-esp32-demo`（ESP32 OLED 再生）
