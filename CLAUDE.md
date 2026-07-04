# TMG1 Studio

@.claude/architecture.md
@.claude/coding-style.md
@.claude/workflows.md
@.claude/context/current-sprint.md
@.claude/context/known-issues.md

## Quick facts
- 種別: クロスプラットフォーム デスクトップ GUI（動画 → 1bit モノクロを**区間ごと**に調整するトランスコーダ）
- FW: Tauri v2（Rust バックエンド + Web フロント）
- フロント: Vanilla TypeScript (~5.6) + Vite 6、`withGlobalTauri: true`
- バックエンド: Rust (edition 2021)。crate `tmg1_studio_lib`
- 外部依存: システムの `ffmpeg` / `ffprobe`（`PATH` 上のものを利用。同梱しない）
- テスト: `cd src-tauri && cargo test`（フィルタチェーンビルダの単体テスト）
- 出力: 形式を選択可（`<name>.raw` packed `monob` / `<name>.tmg1` / 両方）+ 常に `<name>.preview.mp4`。
  `.tmg1` は PATH 上の `tmg1` CLI をサブプロセスで呼んで生成（codec/CLI 本体は本 repo に持ち込まない）
- 位置づけ: TMG1 パイプラインの制作ツール（コーデック本体は `tmg1-codec`、CLI は `tmg1-cli`）

## 関連リポジトリ
- `tmg1-codec`: 共通 C++ コーデックライブラリ
- `tmg1-cli`: Rust 製 CLI（encode/decode/transcode）。Studio の `.raw` を `.tmg1` に変換する
- `tmg1-esp32-demo`: ESP32 OLED リファレンスプレイヤー（再生対象）

## Claudeへの指示
- 方針の決定や修正に関する意図や経緯があれば記録していくこと。
- **コーデックのアルゴリズム詳細（TMG1 / Range コーダ等）は本リポジトリでは扱わない**。
  `tmg1-codec` を参照すること。Studio は monob(1bit) 生成までを担い、TMG1 エンコードは PATH 上の
  `tmg1` CLI にサブプロセスで委ねる（ffmpeg 同様、バイナリは同梱しない）。
- セッションの記録は `session-record` スキルを使う。
