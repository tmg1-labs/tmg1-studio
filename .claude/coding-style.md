# コーディング規約

## 命名規則

### Rust (src-tauri)
- 関数・変数: snake_case（例: `probe_video`, `render_preview`, `build_filter`）
- 型・構造体: PascalCase（例: `VideoInfo`）
- 定数: UPPER_SNAKE_CASE
- Tauri コマンドは `#[tauri::command]` を付け、`invoke_handler` の `generate_handler!` に登録する。

### TypeScript (src)
- 変数・関数: camelCase
- 型・インターフェース: PascalCase
- 定数: UPPER_SNAKE_CASE
- バックエンド呼び出しは `@tauri-apps/api` の `invoke` 経由。コマンド名は Rust 側と一致させる。

## フォーマッタ / Linter
- Rust: `cargo fmt` / `cargo clippy`
- TypeScript: `tsc`（`strict: true` / `noUnusedLocals` / `noUnusedParameters` /
  `noFallthroughCasesInSwitch` 有効。未使用変数・パラメータはビルドを通さない）

## コメント・ドキュメント
- ソースコードには**日本語のコメント**を記入し、仕組みを把握しやすくする。
- フィルタチェーンの各段（contrast / level squeeze / dither）が「何を目的にしているか」を明示する。
- README はライブラリ／ツールの使い方を**英語**で書き、**日本語版**（README.ja.md）も同時に用意する。

## その他のルール
- **フィルタ構築は `filter.rs` に一本化**する。プレビューとエクスポートで別実装を作らない。
- Rust ↔ TS 間はシリアライズ（serde / JSON）で受け渡す。フィールド名の対応を崩さない。
- ffmpeg / ffprobe の呼び出しは `ffmpeg.rs` のラッパを経由し、直接 `Command` を散らさない。
- 出力幅は 8 の倍数（monob バイト境界）を守る。
