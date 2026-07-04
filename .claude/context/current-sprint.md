# 現在の作業コンテキスト

最終更新: 2026-07-04（プリセット/プロジェクト保存/起動フロー/設定メニュー/未保存確認まで実装）

## 今やっていること
- **機能拡充フェーズ**。GUI の編集ワークフローを一通り整備した。時系列（新しい順）:
  - **未保存変更の確認ダイアログ（3択モーダル）**（**未コミット・この後コミット予定**）。
    dirty フラグを編集ハンドラで立て、「閉じる」ボタンとアプリ終了(`onCloseRequested`)の
    両方で `保存する/保存しない/キャンセル` を出す。カスタム HTML モーダル（Tauri の `ask` は
    2択のため）。`onSaveClick` は成功可否を boolean で返し、保存キャンセル時は閉じ/終了を止める。
    `core:window:allow-destroy` 権限を追加。
  - **設定メニュー（歯車）+ バージョン表記**（**未コミット・この後コミット予定**）。
    ツールバー右端に歯車 → ドロップダウンでバージョン表示のみ。バージョンは Vite `define` の
    `__APP_VERSION__`（優先: 環境変数 `VITE_APP_VERSION` → package.json）。**将来のタグ自動
    ビルドで CI がタグから `VITE_APP_VERSION` を渡せば表示が一致する**土台を用意（実証済み）。
    あわせてボタン改名（読み込み→開く / プロジェクトを閉じる→閉じる）。
  - `2ceab24` **プロジェクト保存/読込 + 起動フロー刷新 + FPS既定15**。`.tmgproj`(JSON) に
    入力パス・出力w/h/fps・区間・再生範囲を保存/復元（backend `save_project`/`load_project`）。
    起動時は「新規作成/開く」のみ、開いた後は「保存(新規)/上書き(既存)」+「閉じる」を表示
    （`body.no-project` で切替）。FPS 既定を 30→15、元動画fps自動採用を廃止。
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
- 未検討: 「名前を付けて保存」（別名保存）、tmg1 直エクスポート（現状は monob raw まで）。

## 参考
- パイプライン全体像・関連リポジトリは `CLAUDE.md` / `architecture.md` を参照。
- コーデック本体・CLI の作業状況は `tmg1-codec` / `tmg1-cli` の current-sprint.md を参照。
