# 既知の問題・注意事項

> ハマりやすい地雷の要点を集約する。コーデック実装・CLI 固有の問題は
> `tmg1-codec` / `tmg1-cli` の known-issues.md を参照。

## ハマりやすい箇所

### ffmpeg / ffprobe が PATH に無い
- **症状**: probe_video / render_preview / export が失敗する。
- **原因**: バイナリを同梱せず、システム PATH 上の `ffmpeg` / `ffprobe` を呼ぶ設計のため。
- **回避策**: 実行環境に ffmpeg をインストールし PATH を通す。

### 出力幅が 8 の倍数でない
- **症状**: monob 出力のバイト境界がずれ、実機表示が崩れる。
- **原因**: `monob` は 1 バイト = 8 ピクセルで packing するため、幅は 8 の倍数が前提。
- **回避策**: 素材／設定の幅を 8 の倍数に揃える。

### Vite 固定ポート 1420 が使用中
- **症状**: `npm run tauri dev` が strictPort 設定で起動失敗する。
- **回避策**: 1420 を占有しているプロセスを止める。

### `tauri dev` 実行中に `cargo build` が「アクセスが拒否されました (os error 5)」
- **症状**: 起動中の app が `target/debug/tmg1-studio.exe` をロックしており、`cargo build` が
  最終リンク(exe 差し替え)で失敗する（コードエラーではない）。
- **回避策**: 検証は exe を差し替えない `cargo check` を使う。Rust/権限変更は `tauri dev` が
  自動リビルド・再起動して反映する。

### `@tauri-apps/plugin-store` の `StoreOptions.defaults` が必須（v2.4.3）
- **症状**: `load(path, { autoSave: true })` が型エラー（`defaults` が無い）。
- **回避策/利点**: `load(path, { defaults: { ... }, autoSave: true })` を渡す。この `defaults` は
  初回起動時の seed として使える（プリセットの組み込み初期値をここで与えている）。
  以降はディスク状態が優先され、削除/上書きは保存後ディスクへ反映され永続化される。

### ウィンドウを閉じる確認（onCloseRequested）に権限が要る
- **症状**: `getCurrentWindow().onCloseRequested` で未 prevent 時に内部で `destroy()` が呼ばれるが、
  権限が無いと閉じられない。
- **回避策**: capabilities に `core:window:allow-destroy` を追加する。`onCloseRequested` の handler は
  await されるので、非同期で確認モーダルを出し、キャンセル時のみ `event.preventDefault()` する。

### 表示バージョンは Vite `define` で注入（タグ自動ビルド用の土台）
- **メモ**: `__APP_VERSION__` を `vite.config.ts` の `define` で埋め込む。優先順は
  環境変数 `VITE_APP_VERSION` → package.json。CI でタグから `VITE_APP_VERSION` を渡せば表示が
  タグに一致する。`tsc` 用に `declare const __APP_VERSION__: string;` を宣言しておくこと。

### ffmpeg `-progress` は短い素材だと最終ブロックのみ（バーが 0→100 に飛ぶ）
- **症状**: 再生用レンダリングの進捗バーが、軽い/短い素材だと途中経過なく一気に 100% になる。
- **原因**: ffmpeg の `-progress pipe:1` は一定間隔＋終了時に進捗ブロックを出す。即座に終わる処理では
  終了時の1ブロックしか出ない。重い 4K デコードでは定期的に出るので滑らかに進む（本来の目的ケース）。
- **メモ**: `-progress` の値は `frame=120` のように**空白なし**なので `strip_prefix("frame=").trim().parse` で取れる。
  総フレームは `スライス尺 × fps` の四捨五入合計（推定）なので実値と ±数フレームずれ得る→ 100% にクランプ。

### 組み込みプリセット名の変更は既存ストアに反映されない（seed は初回のみ）
- **症状**: `BUILTIN_PRESETS` の名前を変えても、すでに起動済みの環境では古い名前のまま。
- **原因**: `plugin-store` の `defaults` は**キーが無いときだけ**適用される seed。初回起動で `presets.json` に
  書かれた後は変更が届かない。
- **回避策**: `presets.json`（アプリ data ディレクトリ、例 `%APPDATA%\com.tmg1labs.studio\`）を削除して再 seed。
  なお i18n では**プリセット名は非翻訳**（永続データのため）。翻訳するなら内部キー＋表示名翻訳への移行が要る。

### レンダリングキャッシュの無効化は markDirty に紐付け
- **メモ**: 再生用レンダリング結果(`objectUrl`)は `renderValid` で有効性を管理し、`markDirty`（範囲/
  パラメータ/区間/出力の編集）で無効化。**スクラブ（playhead 移動）は markDirty しない**ので、停止→
  スクラブ→再生ではキャッシュを維持して再レンダリングしない（意図どおり）。停止は一時停止で Blob を保持し、
  完全破棄は `discardRender`（プロジェクト読込/クローズ時のみ）。

## 地雷・禁止事項
- プレビューとエクスポートでフィルタチェーン構築を分岐させない（WYSIWYG が崩れる。
  修正は `filter.rs` に一本化する）。
- 表示文字列をハードコードしない（i18n。静的は `data-i18n`、動的は `t(key, params)`。言語追加は
  `locales/xx.json` + `i18n.ts` に1行）。
- TMG1 / コーデックのアルゴリズムを本リポジトリに持ち込まない（責務は `tmg1-codec` / `tmg1-cli`）。
