# 現在の作業コンテキスト

最終更新: 2026-07-05（テスト/チェック CI 実装＋GitHub Actions 実走 green・annotations 0 確認。push 済み `cf82ace`/`9c04944`/`5c15f85`）

## 今やっていること
- **テスト/チェック CI（GitHub Actions）実装**（2026-07-05、ローカル検証済み・**未コミット**）。
  - 方針（ユーザー選択）: (1) 範囲=**テスト/チェック CI をまず先に**（リリース CI は別途）、
    (2) clippy は**4件修正＋ゲート維持＋ツールチェーン固定**。
  - `.github/workflows/ci.yml` 新規（push=main/feature/**・pull_request）。ubuntu-latest 単一ジョブ `check`:
    Tauri の Linux ビルド依存を apt 導入（`libwebkit2gtk-4.1-dev`/`libgtk-3-dev`/`librsvg2-dev`/
    `libxdo-dev`/`libssl-dev`/`patchelf`）→ `npm ci`＋`npm run build`（tsc --noEmit + vite build）→
    `Swatinem/rust-cache`（workspaces: src-tauri）→ `cargo test`→`cargo clippy --all-targets -- -D warnings`。
    リリース用インストーラのビルドはしない（チェック専用）。
  - `src-tauri/rust-toolchain.toml` 新規で **Rust を `1.96.0` に固定**（`components=["clippy","rustfmt"]`）。
    浮動 stable だと新 clippy リリースで `-D warnings` が突然赤くなるためドリフト防止。
  - clippy 指摘 4件を `src-tauri/src/ffmpeg.rs` で解消（`x % 8 != 0`→`!x.is_multiple_of(8)` 2件、
    手動チェック付き除算→`checked_div` 2件）。**バグ修正ではなくスタイル系 lint の解消**。
  - **検証**: ローカルで `npm run build`✅ / `cargo test`（2 passed）/ `cargo clippy --all-targets -D warnings`
    EXIT 0。**GitHub Actions 実走も green 確認済み**（初回 6m44s→rust-cache 効き2回目 1m49s）。
  - **annotations 0 化**（commit `5c15f85`）: 初回実走で `actions/setup-node@v4` に Node20 deprecation
    警告が付いたため **v5（runs.using=node24）へ更新**。esp32-demo で setup-python/cache を v6 化したのと
    同じ annotations 0 方針に統一。`setup-node` は v5 が Node24 対応（v6 も存在するが v5 で解消）。

- **GitHub リモート作成 ＋ 初回 push 完了**（2026-07-05、push 済み）。
  - リモート `origin` = `https://github.com/tmg1-labs/tmg1-studio.git`（他の兄弟リポと同じ HTTPS、
    組織は**複数形 `tmg1-labs`**。ユーザー指定の "tmg1-lab" 単数形は実在せず `tmg1-labs` が正）。
  - 空リポへ `main` を push、upstream 追跡設定済み。これで「リモート未設定」制約は解消。

- **エクスポート進捗のプログレスバー表示**（2026-07-05、実装・`tsc` 通過／コミット済み `402c586`）。
  - 方針（ユーザー選択）: (1) 粒度=**区間単位ステップ**（既存 `export-progress {done,total}` を流用）、
    (2) 表示=**既存の再生用バー `render-progress` を流用**、(3) tmg1 エンコード工程（フレーム進捗なし）は
    **不定（パルス）表示**。
  - 変更: `index.html`（バーの label/track に id 付与）、`styles.css`（`.render-progress-track.indeterminate`
    のスライドアニメ `@keyframes render-progress-slide`）、`main.ts`（`showRenderProgress(labelKey)` で
    ラベル差替、`setRenderProgress` は毎回 indeterminate 解除、`setRenderProgressIndeterminate(labelKey)`
    追加。`doExport` 開始で `showRenderProgress("exporting")`、`export-progress` で done/total→0–100%、
    `tmg1-encoding` で不定表示、`finally` で `hideRenderProgress`）。**backend 無変更**（既存イベント活用）。
  - バーは再生レンダリングと共用（同時実行なし）。ラベルは表示のたび動的セット＝セッション中の言語切替に追従。
  - i18n キー追加不要（`exporting`/`tmg1Encoding`/`rendering` は既存）。
  - **GUI 実操作での見た目（ステップ→パルスの動き）確認済み（2026-07-05、ユーザー確認）**。

- **エクスポート結果レポートのモーダル表示**（2026-07-05、実装・`cargo test`/`tsc` 通過／コミット済み `800f2dc`）。
  - 方針（ユーザー選択）: (1) 完了時に**専用モーダル**ポップアップ、(2) **全体サマリのみ**（区間内訳なし）、
    (3) **サイドカー出力なし**（画面表示のみ）。
  - 圧縮率は `tmg1` CLI が出さないため Studio 側で算出。backend `export()` が集計済みの未圧縮 raw 総バイトと
    tmg1 出力ファイルサイズ（`fs::metadata`）から算出する。
  - 変更: `ffmpeg.rs`（`ExportResult` に `raw_bytes: u64` / `tmg1_bytes: Option<u64>` 追加。tmg1 化成功後に
    metadata 取得）、`main.ts`（`showExportReport`/`formatBytes`/`buildEncodeSummary`/`addReportRow`、
    完了時に呼出）、`index.html`（`#report-modal` + `dl.report-list`）、`styles.css`（`.report-list` グリッド、
    値は等幅・path は改行維持折返し）、locales 3言語に `report*` キー12種追加。
  - 表示項目: 解像度/fps/フレーム数/尺/区間数/形式/RAW サイズ/TMG1 サイズ/圧縮率(%と N:1)/平均B/frame/
    エンコード設定/出力パス。**RAW のみエクスポート時は圧縮系の行を隠す**。閉じる/オーバーレイ/Esc で閉じる。
  - **GUI 実操作での表示・実ファイルサイズ一致目視 確認済み（2026-07-05、ユーザー確認）**。

## 過去にやったこと
- **プレビューmp4を任意出力化**（2026-07-05、実装・検証済み／コミット済み `491cd85`）。
  - 背景: 従来はエクスポート形式に関わらず常に `.preview.mp4` を生成していた。ユーザー判断で
    「必要なときだけ出す」方針に変更（既定オフ）。既定では `.tmg1`/`.raw` のみ出力。
  - 変更: backend `export()` に `preview: bool` を追加し true のときだけ mp4 生成、
    `ExportResult.mp4_path` を `Option` 化（lib.rs も引数追加）。front はエクスポート設定ダイアログに
    「プレビューmp4も出力」チェック（既定オフ）を追加、`state.exportPreview` を `.tmgproj`(v2、欠落は
    false 補完)に保存・復元。完了メッセージは mp4 有無で出し分け（`exportDoneNoPreview` 追加）。
    docs(architecture/CLAUDE) も「指定時のみ・既定オフ」に更新。
  - 検証: `cargo check`/`cargo test`、`tsc --noEmit`、`vite build` 通過、locale JSON 3言語 valid。

- **編集画面の UI 改善一式**（2026-07-05、実装・確認済み／コミット済み `a4157fb`）。
  - ツールバー: ボタンを非収縮・折り返し防止（狭幅でも変形しない）、保存/閉じるをエクスポートの左へ移動。
  - ファイル情報: パスは**中間省略**（dir を末尾省略＋ファイル名は常時表示、ホバーで title にフルパス）。
    元動画スペックはパス右の**情報アイコン(i)のホバーで自作ツールチップ表示**（ネイティブ title は付けない＝
    二重表示回避。アイコン字体の Georgia 斜体はグリフのみに限定しツールチップへ非継承）。
  - ウィンドウタイトルに編集中のプロジェクト名（未保存は動画名）を表示（`setTitle`。capability に
    `core:window:allow-set-title` 追加＝**Rust 側再ビルドが要る**）。
  - タイムライン: 区間クリックで**始点**にシーク（従来は中央＝コメントとの不一致バグ）。スクラブは赤線を
    トラック両端(0–100%)に届かせ、つまみを左右に半径分はみ出させて**つまみ中心が赤線に一致**
    （`.scrub width:calc(100%+14px); margin:… -7px`、つまみ14px 固定）。
  - レイアウト: `.preview-pane/.params-pane` に `overflow-y:auto`（縦を縮めても区間パラメータ列が突き抜けない）。
  - 検証: `tsc --noEmit` / `vite build` 通過、locale JSON 3言語 valid。

- **tmg1 エンコードパラメータの GUI 露出 ＋ エクスポート設定ダイアログ ＋ encode プリセット**
  （2026-07-05、**実装・検証・GUI 実操作確認済み／コミット済み `e23e1dc`**）。
  - 背景: 前タスクの tmg1 直エクスポート（`8e8fb91`）はエンコードが CLI 既定固定で `--size`/`--fps`
    しか渡していなかった。素材ごとに圧縮/画質を調整できるよう、エクスポート時にダイアログを出して
    encode パラメータを触れるようにし、専用プリセット保存＋`.tmgproj` 保存で再現可能にした。
  - 方針（ユーザー選択）: (1) 露出は**主要＋詳細**（coder/rice-mode/rice-k/key-int/scd/vfr＋折りたたみで
    prediction/delta/index）。**msb-first/invert は非露出**（monob 前提で固定。触ると実機表示が壊れる）。
    (2) 永続化は `.tmgproj`(version 2、欠落は既定補完)＋区間用と別の encode 専用プリセット
    (`encode-presets.json`)。(3) ツールバーの形式セレクタ `#out-format` はダイアログに集約。
  - 変更: backend `ffmpeg.rs`（`Tmg1Encode` 構造体＝serde default で CLI 既定に一致・`Project` に
    `#[serde(default)] encode` 追加・`encode_tmg1()` にフラグ条件付与）。lib.rs は Project 経由のため無改修。
    front `index.html`（`#export-modal` 新設・`#out-format` 移設）・`styles.css`（`.modal.wide`＋フォーム行）・
    `main.ts`（`Tmg1Encode`/`DEFAULT_TMG1_ENCODE`/`EncodePreset`/`BUILTIN_ENCODE_PRESETS` 型・state.encode/
    exportFormat・ProjectFile v2・`askExportSettings` Promise ダイアログ・`doExport` 改修・coder/rice-mode 連動
    disable・encode プリセット一式 `initEncodePresets`）・locales 3言語に encode 系キー追加。
  - **CLI フラグ仕様（重要）**: bool は値付き `--scd true|false` 等（`BoolishValueParser`+`num_args=1`）。
    `--index` は真偽フラグ（真のときだけ付与、値を付けるとエラー）。`--fps` は u16（f64 を round して渡す）。
    coder/rice-mode は kebab（rice/range, fixed/per-line/per-frame）。
  - **検証**: `cargo check`/`cargo test`（filter 2 テスト）通過、`tsc --noEmit` EXIT=0、`vite build` 成功、
    locale JSON 3言語 valid、実 `tmg1` で非既定組合せ（`--coder range --scd false --index` 等・rice fixed k=3）を
    **encode→decode ロスレス往復 OK**。**GUI 実操作（ダイアログ表示・プリセット保存/適用・エクスポート・
    プロジェクト再読込での復元）は未確認**。

- **tmg1 直エクスポート実装**（2026-07-05、実装・検証・実機再生確認済み／コミット済み `8e8fb91`＋`20bca15`）。
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
    ロスレス往復 OK**。**GUI 実操作での tmg1 エクスポート→ESP32 実機再生も確認済み（2026-07-05、
    ユーザー確認）** ＝ 実機再生まで含めて完了。
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
- リモート = `tmg1-labs/tmg1-studio`（push 済み）。CI は未整備。
- CI 未整備。テストは手元で `cargo test`（filter.rs のユニットテスト）。
- ffmpeg/ffprobe は PATH 前提。実行環境に無いと probe/preview/export が失敗する。
- `tauri dev` 実行中は exe ロックで `cargo build` が「アクセス拒否」になる → 検証は `cargo check`。

## 決定事項（やらないと決めたこと）
- **組み込みプリセット名の翻訳はしない**（2026-07-05 ユーザー判断で不要）。現状の非翻訳・英語名固定の
  ままとする（永続データとの不整合を避けるため、内部キー＋表示名翻訳への移行はしない）。

## 次にやること
- タグ駆動の自動ビルド/リリース CI（GitHub Actions）。その際 `VITE_APP_VERSION` にタグを渡し、
  installer 用に tauri.conf.json / Cargo.toml の version もタグへ同期する構成にする。
- 未検討: 「名前を付けて保存」（別名保存）。

## 参考
- パイプライン全体像・関連リポジトリは `CLAUDE.md` / `architecture.md` を参照。
- コーデック本体・CLI の作業状況は `tmg1-codec` / `tmg1-cli` の current-sprint.md を参照。
