# 現在の作業コンテキスト

最終更新: 2026-07-05（tmg1 直エクスポート実装。検証済み・**未コミット**）

## 今やっていること
- **tmg1 直エクスポート実装**（2026-07-05、**実装・検証済み／コミット直前**）。
  - 背景: 従来は monob raw までで、`.tmg1` 化は手作業で `tmg1-cli` に raw を渡す運用だった
    （下記「次にやること」に未検討として挙がっていた項目）。Studio からワンステップで `.tmg1` を
    出せるようにした。
  - 方針（ユーザー選択）: (1) `tmg1` CLI を**サブプロセス**起動（PATH 参照。ffmpeg 非同梱・codec を
    本 repo に持ち込まない禁止パターンと整合）、(2) 出力形式を**選択式**（raw / tmg1 / 両方。
    `.preview.mp4` は常時生成）、(3) エンコードパラメータは**CLI 既定のみ**（`--size`/`--fps` だけ渡す）。
  - 変更: backend `ffmpeg.rs`（`ExportFormat` 追加・`encode_tmg1()` 新設＝`tmg1 encode --size WxH
    --fps N -i <raw> -o <tmg1>`・`export()` を形式対応に改修・`ExportResult` に `tmg1_path` 追加＆
    `raw_path` を Option 化）、`lib.rs`（`export` に `format` 引数）。front `index.html`（形式セレクタ
    `#out-format`、既定 tmg1）・`main.ts`（`doExport` 分岐・保存ダイアログ拡張子/フィルタ切替・
    `tmg1-encoding` イベント表示・`ExportResult` 型更新）・locales 3言語（`format`/`formatTmg1`/
    `formatBoth`/`formatRaw`/`dialogTmg1Filter`/`tmg1Encoding` 追加、`exportDone` の `{raw}`→`{out}`）。
    ドキュメント（CLAUDE.md/architecture.md/known-issues.md）も lib_deps ではなくサブプロセス方針で更新。
  - **検証**: `cargo check`/`cargo test`（filter 2テスト）通過、`tsc --noEmit` EXIT=0。実 `tmg1`
    （`~/.cargo/bin/tmg1.exe`）で backend と同一の `encode` コマンドを実行し **encode→decode
    ロスレス往復 OK**。GUI 全体を通した実操作での目視（クリップ→tmg1→ESP32 再生）は未実施。
  - tmg1 のみ選択時は連結 raw を一時ファイルにして encode 入力にのみ使い、完了後に削除する。

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
- tmg1 直エクスポートの **GUI 実操作での目視確認**（クリップ→形式=tmg1→ESP32 再生）。コマンドライン
  経路は検証済み。あわせて form=both で `.raw` が従来と同一バイトになる回帰確認。
- 未検討: 「名前を付けて保存」（別名保存）、tmg1 エンコードパラメータの GUI 露出（現状は CLI 既定固定。
  coder/rice-mode/scd/vfr/key-int 等）、組み込みプリセット名の翻訳（現状は永続データ都合で非翻訳の
  英語名。内部キー＋表示名翻訳へ移行が必要）。

## 参考
- パイプライン全体像・関連リポジトリは `CLAUDE.md` / `architecture.md` を参照。
- コーデック本体・CLI の作業状況は `tmg1-codec` / `tmg1-cli` の current-sprint.md を参照。
