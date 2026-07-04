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

## 地雷・禁止事項
- プレビューとエクスポートでフィルタチェーン構築を分岐させない（WYSIWYG が崩れる。
  修正は `filter.rs` に一本化する）。
- TMG1 / コーデックのアルゴリズムを本リポジトリに持ち込まない（責務は `tmg1-codec` / `tmg1-cli`）。
