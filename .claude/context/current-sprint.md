# 現在の作業コンテキスト

最終更新: 2026-07-04（i18n・プレビュー角丸除去・再生レンダリング進捗バー・レンダリングキャッシュまで実装。全てコミット済み）

## 今やっていること
- **機能拡充フェーズ**。GUI の編集ワークフローを一通り整備した。時系列（新しい順・すべてコミット済み）:
  - `3136fbb` **停止→再生で毎回レンダリングが走る問題を修正（レンダリングキャッシュ）**。
    `renderValid` フラグを導入。レンダリング成功で true、`markDirty`（範囲/パラメータ/区間/出力の編集）で false。
    `stopPlayback` を一時停止化し Blob(video.src) を保持、`startPlayback` は valid なら再レンダリングせず再開。
    完全破棄は `discardRender`（プロジェクト読込/クローズ時のみ）。
  - `3d82b6e` **再生用レンダリングのフレーム単位進捗バー**。`render_range` の各スライスで ffmpeg に
    `-progress pipe:1 -nostats` を付け stdout の `frame=` を解析、範囲総フレームに対する累積で 0–100% を
    `range-progress` イベント送出。フロントはプレビュー上のオーバーレイバーで表示。
  - `25c06c7` プレビュー表示域の角丸除去（`.preview-frame` の border-radius 0）。
  - `f34d961` **多言語対応(i18n) 日本語/英語/簡体中文**。自前 i18n（`src/i18n.ts` + `locales/*.json`）。
    静的テキストは `data-i18n`、動的は `t(key, params)`。設定メニューに言語セレクタ、`settings.json`(store)で
    永続化。組み込みプリセット名を英語化（永続データとの翻訳不整合回避）。
  - `e8ec16a` 設定メニュー(歯車+バージョン) + 未保存変更の3択確認モーダル（保存する/保存しない/キャンセル、
    閉じる・アプリ終了 onCloseRequested 両対応）。`core:window:allow-destroy` 追加。バージョンは Vite
    `define` の `__APP_VERSION__`（`VITE_APP_VERSION` → package.json）。
  - `2ceab24` プロジェクト保存/読込 + 起動フロー刷新（`body.no-project`）+ FPS既定15。`.tmgproj`(JSON) に
    入力パス・出力w/h/fps・区間・再生範囲を保存/復元（backend `save_project`/`load_project`）。
  - `a167dcd` 再生範囲コントロール（始点/終点に設定・範囲解除）+ ハンドル拡大 + 区間ラベル白字化。
  - `429e074` 区間パラメータのプリセット機能（`tauri-plugin-store` で永続化、組み込み4種を seed）。
  - `6cb5968` Claude Code 用プロジェクト設定（CLAUDE.md + .claude/）追加。

## 一時的な制約・注意事項
- **リモート未設定**。ローカルコミットのみ（GitHub へ未 push）。
- CI 未整備。テストは手元で `cargo test`（filter.rs のユニットテスト）。
- ffmpeg/ffprobe は PATH 前提。実行環境に無いと probe/preview/export が失敗する。
- `tauri dev` 実行中は exe ロックで `cargo build` が「アクセス拒否」になる → 検証は `cargo check`。

## 次にやること
- タグ駆動の自動ビルド/リリース CI（GitHub Actions）。その際 `VITE_APP_VERSION` にタグを渡し、
  installer 用に tauri.conf.json / Cargo.toml の version もタグへ同期する構成にする。
- GitHub リモート作成 + 初回 push。
- 未検討: 「名前を付けて保存」（別名保存）、tmg1 直エクスポート（現状は monob raw まで）、
  組み込みプリセット名の翻訳（現状は永続データ都合で非翻訳の英語名。内部キー＋表示名翻訳へ移行が必要）。

## 参考
- パイプライン全体像・関連リポジトリは `CLAUDE.md` / `architecture.md` を参照。
- コーデック本体・CLI の作業状況は `tmg1-codec` / `tmg1-cli` の current-sprint.md を参照。
