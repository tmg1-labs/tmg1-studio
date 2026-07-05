# 既知の問題・注意事項

> ハマりやすい地雷の要点を集約する。コーデック実装・CLI 固有の問題は
> `tmg1-codec` / `tmg1-cli` の known-issues.md を参照。

## ハマりやすい箇所

### ffmpeg / ffprobe が PATH に無い
- **症状**: probe_video / render_preview / export が失敗する。
- **原因**: バイナリを同梱せず、システム PATH 上の `ffmpeg` / `ffprobe` を呼ぶ設計のため。
- **回避策**: 実行環境に ffmpeg をインストールし PATH を通す。

### tmg1 直エクスポートには `tmg1` CLI が PATH に必要
- **症状**: 形式に「tmg1」「両方」を選んだ export が「tmg1 CLI が見つかりません」で失敗する。
- **原因**: ffmpeg と同じく `.tmg1` 生成は PATH 上の `tmg1` バイナリをサブプロセスで呼ぶ設計（非同梱）。
- **回避策**: `tmg1-cli` を `cargo install --path ../tmg1-cli`（または PATH の通る場所へ配置）で用意する。
  raw のみの export は `tmg1` 不要。

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

### `display` を付けた要素は `hidden` 属性が効かなくなる
- **症状**: `.info-badge { display: inline-flex }` を当てると、JS で `el.hidden = true` にしても消えない。
- **原因**: UA の `[hidden]{display:none}` と著者スタイルの `.class{display:…}` は同特異性で、著者スタイルが勝つ。
- **回避策**: 明示的に `.info-badge[hidden] { display: none; }` を書く。JS で hidden トグルする要素に
  `display` を当てるときは必ずセットで用意する。

### ネイティブ `title` と自作ツールチップの二重表示
- **症状**: 情報アイコンにアクセシビリティ用 `title`（data-i18n-title）と CSS 自作ツールチップを両方付けると、
  ホバー時にブラウザ標準ツールチップと自作ツールチップが重なって出る。
- **回避策**: どちらか一方に統一する（今回は `title` を外し自作ツールチップのみ）。斜体セリフ字体などの
  装飾は親（バッジ）ではなく対象グリフ（`.info-icon`）だけに当て、ツールチップへ継承させない。

### range input のつまみ中心をタイムラインの縦線に合わせる
- **メモ**: `<input type=range>` のつまみ中心の可動域は `[R, W-R]`（R=つまみ半径）で 0/W の端に届かない。
  タイムライン側の縦線（`left:0%〜100%`）と一致させるには、**スクラブを左右に R はみ出させて**
  可動域を 0%〜100% に広げる（`.scrub width:calc(100%+2R); margin:… -R`、つまみ幅=2R を固定）。
  逆に縦線をつまみ中心へ寄せる方式だと縦線が端に届かなくなる。

### `setTitle` 等の window API は capability 追加＝Rust 側再ビルドが必要
- **症状**: `getCurrentWindow().setTitle()` などを追加しても、capability 未追加だと権限エラーで無反応。
- **回避策**: `capabilities/default.json` に `core:window:allow-set-title` 等を追加。**capabilities 変更は
  フロント HMR では反映されず**、`tauri dev` の Rust 再ビルド（実質アプリ再起動）が要る。

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
