# 既知の問題・注意事項

> ハマりやすい地雷の要点を集約する。コーデック実装・CLI 固有の問題は
> `tmg1-codec` / `tmg1-cli` の known-issues.md を参照。

## ハマりやすい箇所

### ffmpeg / ffprobe が PATH に無い
- **症状**: probe_video / render_preview / export が失敗する。
- **原因**: バイナリを同梱せず、システム PATH 上の `ffmpeg` / `ffprobe` を呼ぶ設計のため。
- **回避策**: 実行環境に ffmpeg をインストールし PATH を通す。**または設定メニューの「実行ファイルのパス」で
  ffmpeg/ffprobe の絶対パスを指定する**（2026-07-05 追加。下記「実行パスの設定は Rust 側で store 直読み」参照）。

### tmg1 直エクスポートには `tmg1` CLI が PATH に必要
- **症状**: 形式に「tmg1」「両方」を選んだ export が「tmg1 CLI が見つかりません」で失敗する。
- **原因**: ffmpeg と同じく `.tmg1` 生成は PATH 上の `tmg1` バイナリをサブプロセスで呼ぶ設計（非同梱）。
- **回避策**: `tmg1-cli` を `cargo install --path ../tmg1-cli`（または PATH の通る場所へ配置）で用意する。
  raw のみの export は `tmg1` 不要。**または設定メニューの「実行ファイルのパス」で tmg1 の絶対パスを指定する**
  （2026-07-05 追加）。

### 実行パスの設定は Rust 側で store 直読み（invoke 署名を増やさない）
- **メモ（設計）**: ffmpeg/ffprobe/tmg1 の実行パスは設定メニューでフロントが `settings.json`
  （tauri-plugin-store、言語設定と共用）へ `ffmpegPath`/`ffprobePath`/`tmg1Path` として保存し、
  **Rust 側が `StoreExt::store("settings.json")` で直読み**する（`ExePaths::load(app)`）。invoke の引数を
  増やさない代わりに、`probe_video`/`render_preview` は `AppHandle` 引数を足した（Tauri が自動注入するため
  フロントの呼び出しコードは無変更）。空文字/未設定キーは既定（PATH 上のコマンド名）へフォールバック。
- **落とし穴（キー名の一致）**: store のキー名は **Rust の `ExePaths::load` と front の `EXE_PATH_KEYS`
  で文字列一致必須**（`ffmpegPath` 等）。片方だけ変えると黙って PATH 既定に落ちる（無反応に見える）。
- **メモ（即時反映）**: 呼び出しごとに `ExePaths::load`（store 読込）するので設定変更は再起動不要で次の
  probe/export から効く。store 読込は軽量なので毎回でも問題なし。

### `project-only` の中に置いた要素は「プロジェクト未読込」時に丸ごと消える（2026-08-13 被害あり）
- **症状**: 新規作成／開く でファイルを選んだ後、**画面が一切変化しない**。エラーも出ない。
- **原因**: `<p id="status">` が `<footer class="timeline-pane project-only">` の中にあり、
  `body.no-project .project-only { display:none !important }` で**起動直後（no-project）は
  ステータスバーごと非表示**だった。`probe_video` が失敗してもエラーは DOM に書かれるだけで、
  画面に出る場所が存在しなかった（＝「無反応」に見える）。
- **回避策**: 常時見せたい要素（ステータスバー・警告バー）は `project-only` の**外**に置く。
  2026-08-13 に `#status` を footer の外へ移し、`#tool-warning` も同じ階層に追加した。
- **教訓**: 「無反応」を疑ったら、まず **DOM の状態と画面の表示が一致しているか**を確かめる。
  DOM にエラーが入っているのに画面に出ていなければ、原因は表示側（CSS の可視性）にある。

### 外部ツールの有無チェックは「起動できたか」で見る（終了コード／`--version` に頼らない）
- **メモ（設計）**: `ffmpeg.rs` の `check_tools()` は各実行ファイルを起動できたかだけを見る。
  ffmpeg/ffprobe は `-version` で 0 を返すが、**`tmg1` はサブコマンド必須で `--version` を
  受け付けない**（`error: unexpected argument '--version' found`）ため、終了コードで判定すると
  「存在するのに無い」と誤判定する。`tmg1` には `--help` を渡している。
- 呼び出しは起動時と**実行パス設定の変更時**（`persistExePath`）。言語切替では再チェックせず、
  キャッシュした結果を `renderToolWarning()` で貼り直すだけにする（プロセス再起動を避けるため）。

### アプリは起動した親プロセスの PATH を継承する（PATH を絞ったシェルから起動すると ffprobe 不在）
- **症状**: 普段は動くのに、ある起動のときだけ `ffprobe を実行できませんでした: program not found`。
- **原因**: `ffmpeg`/`ffprobe` は winget 管理なら `%LOCALAPPDATA%\Microsoft\WinGet\Links` にあり、
  これは**ユーザー PATH** の側にある。マシン PATH しか持たない親（サンドボックス化されたシェル等）
  から起動すると子プロセスが解決できない。Explorer から起動した場合はユーザー PATH を含むので通る。
- **回避策**: 起動元の環境を疑う。恒久対策としては設定メニューの「実行ファイルのパス」で絶対パスを指定する。
  2026-08-13 追加の起動時プリフライト警告バーで、この状態は起動直後に画面で分かるようになった。

### Tauri アプリの実機診断は WebView2 の CDP を開けて DOM を直接読む（2026-08-13 に実証）
- **手法**: 環境変数 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` を
  セットしてアプリを起動すると、`http://127.0.0.1:9222/json/list` から CDP ターゲットが取れる。
  WebSocket で `Runtime.evaluate`（`awaitPromise:true`）を投げれば、**リリースビルドでも**
  `window.__TAURI__.core.invoke(...)` を直接叩けるし、DOM の実状態も読める（`withGlobalTauri: true` 前提）。
- **勘所**:
  - 式は**ファイルに書いて渡す**。Git Bash 経由だと引数のバックスラッシュが潰れ、`D:\tmp\...` が
    `D:<TAB>mp...` になって「ffprobe: Invalid argument」のような**偽の失敗**を作る。
  - ネイティブのファイルダイアログはクラス `#32770` の別ウィンドウ。`EnumWindows` で見つけて
    `SetForegroundWindow` → SendKeys でパス入力＋`{ENTER}`（メインウィンドウを AppActivate すると
    ダイアログからフォーカスが外れて入力が届かない）。
  - 結果を `window.__diag` 等に残して後からポーリングすると、ユーザー操作待ちで接続を占有しない。
  - 画面の実描画と DOM の突き合わせには `CopyFromScreen`（HDR は Win+Alt+B でオフ、実枠は
    `DwmGetWindowAttribute(9)`。詳細は下の「スクショ撮影の勘所」）。
- **後片付け**: 診断が済んだらアプリを終了する（9222 を開けたままにしない）。

### `sync-version.mjs` をローカルで実行すると JSON が全行差分になる（CRLF → LF）
- **症状**: 版を上げようと `node scripts/sync-version.mjs 0.1.2` を実行すると、`package.json`
  23行・`tauri.conf.json` 39行が丸ごと差分になる（実質は version 1行のはずなのに）。
- **原因**: リポジトリの JSON は **CRLF** だが、スクリプトは `JSON.stringify(obj, null, 2) + "\n"`
  で書き戻すため **LF** になる。`.gitattributes` が無いので正規化もされない。
  `npm install --package-lock-only` も `package-lock.json` を同様に書き換える（2868行差分になった）。
- **回避策**: **CI 専用と割り切る**（タグビルドでは commit しないので改行差分は無害）。
  ローカルで版を上げてコミットするときは、**version 行だけを直接編集**する。対象は
  `package.json` / `package-lock.json`（root と `packages[""]` の2箇所）/ `src-tauri/tauri.conf.json` /
  `src-tauri/Cargo.toml`。`Cargo.lock` は `cargo check` で1行だけ更新される（こちらは差分が小さい）。

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

### マルチリポ workspace で git が別リポに走る（cd を明示しないと primary へ）
- **症状**: `git commit` が tmg1-studio ではなく別リポ（primary の tmg1-esp32-demo）で実行され、
  「no changes added」や無関係ファイルが対象になる。実際にコミットが誤リポで空振りした。
- **原因**: この workspace は tmg1-esp32-demo が primary、tmg1-studio は additional。
  `cd tmg1-studio && git ...` の**複合コマンドで cd した cwd は次のコマンド呼び出しに持ち越されない**
  ため、cd 無しの git は primary リポで走る。
- **回避策**: tmg1-studio に対する git は**毎回 `cd /d/workspace/TsuMuGi/tmg1-studio && git ...` を明示**。
  実行前に `git status` を挟んで対象リポを確認すると安全。

### CI: Tauri の Rust 側は Linux で system-deps 導入が要る／clippy -D warnings はツールチェーン固定必須
- **メモ（CI 構成）**: `.github/workflows/ci.yml`（push=main/feature/**・PR）の単一ジョブ `check`。
  Tauri アプリは `tauri` クレート依存のため **Linux でのビルド（`cargo test`/`clippy` 含む）に system-deps が必要**
  → apt で `libwebkit2gtk-4.1-dev`/`libgtk-3-dev`/`librsvg2-dev`/`libxdo-dev`/`libssl-dev`/`patchelf` を導入
  （Ubuntu 24.04 ランナー想定のパッケージ名）。フロントは `npm run build`（tsc --noEmit + vite build）で検証。
- **落とし穴（clippy ドリフト）**: `cargo clippy --all-targets -- -D warnings` は clippy のバージョン依存で、
  浮動 stable だと**新 Rust リリースの新 lint で突然赤くなる**。→ `src-tauri/rust-toolchain.toml` で
  `channel="1.96.0"`＋`components=["clippy","rustfmt"]` に**固定**。上げるときは先にローカルで
  `cargo clippy --all-targets -- -D warnings` を通してから channel を上げること。
- **実績**: 初回実走 green（6m44s）。`Swatinem/rust-cache`（workspaces: src-tauri）で2回目 1m49s に短縮。

### GitHub Actions: Node20 deprecation 警告（setup-node は v5 で node24）
- **症状**: 実走の annotation に「Node.js 20 is deprecated ... forced to run on Node.js 24」（`actions/setup-node@v4`）。
  失敗ではないが警告が付く。
- **回避策**: **`setup-node@v5`（`runs.using: node24`）へ更新**で annotations 0。esp32-demo が
  setup-python/cache を v6 化したのと同じ方針。**「最新メジャー=Node24 とは限らない」ので上げる前に
  `runs.using` を実確認**（`gh api repos/actions/setup-node/contents/action.yml?ref=v5 --jq .content | base64 -d | grep -A2 runs:`）。
  setup-node は v5 で node24（v6 も存在するが v5 で解消）。

### リリース CI: Org の Workflow permissions が read-only だと Release 作成が 403（グレーアウトの正体）
- **症状**: リポジトリの Settings → Actions → 「Workflow permissions」で **Read and write がグレーアウト**して
  選べない。read-only のままだと `tauri-action`（や `gh release create`）の Release 作成が
  403「Resource not accessible by integration」で落ちうる。
- **原因**: `tmg1-labs` は **Organization** で、Org の Actions 設定が配下リポジトリを enforce するとリポジトリ側が
  グレーアウト（変更不可）になる。
- **回避策**: Org オーナーが `https://github.com/organizations/tmg1-labs/settings/actions` の
  「Workflow permissions」を Read and write にする（実施済み 2026-07-05）。加えてワークフロー先頭の
  `permissions: contents: write` は Org が read/write 昇格を許可する範囲でジョブ単位に効く（release.yml は宣言済み）。

### リリース CI: テストタグは数値のみの semver にする（MSI/WiX が接尾辞を受け付けない）
- **症状/懸念**: `v0.0.0-test` のようにプレリリース接尾辞付きタグを使うと、Windows の MSI(WiX) が
  version を数値 `x.y.z` しか受け付けず、ワークフロー本体と無関係にビルドが落ちうる。
- **回避策**: 実走テストは**数値のみの使い捨てタグ**（今回 `v0.0.99`。実リリースと衝突しない値）を使う。
  `sync-version.mjs` は `^\d+\.\d+\.\d+` を通すため接尾辞自体は素通しする＝ガードにならない点に注意。
  検証後は `gh release delete <tag> --cleanup-tag --yes`（Release＋リモートタグを一括削除）＋ローカル `git tag -d`。

### リリース CI: tauri-action はマトリクス各脚が同一 tagName のドラフトを共有して添付
- **メモ（設計）**: `release.yml` は 3 プラットフォームを 1 ジョブのマトリクスで回し、各脚が
  `tauri-apps/tauri-action@v0` を同一 `tagName` で呼ぶ。tauri-action は既存の（ドラフト）Release を
  再利用して自分のバンドルを add する冪等動作なので、3 脚が 1 つのドラフトに全 7 バンドルを集約する。
  `releaseDraft: true` で完成後に手動公開。version 同期は各脚の `sync-version.mjs` 実行でタグ値に揃う。
- **実績**: 初回（キャッシュ無し）で ubuntu-22.04 6m3s / windows-latest 6m56s / macos-latest 3m21s。
  `targets: "all"` により Linux は .AppImage/.deb に加え **.rpm も生成**される。

### mkdocs-static-i18n: suffix 構成は言語間リンクミスを誘発する（folder 構成が安全）
- **症状**: 日本語ページ（`index.ja.md`）内のリンクを `getting-started.md` と書くと**英語ページに飛ぶ**。
- **原因**: suffix 構成ではローカライズ版へのリンクは `getting-started.ja.md` と suffix 付きで書く必要がある。
  書き忘れてもビルドは通る（英語版として有効なリンクのため `--strict` でも検出されない）。
- **回避策**: **`docs_structure: folder`**（`docs/en/`・`docs/ja/`）にする。同一フォルダ内の相対リンク
  （`getting-started.md`）が自言語に解決されるので、この種のミスが構造的に起きない（2026-07-06 に移行済み）。
  nav は言語プレフィックスなしのパス（`index.md` 等）で書き、ja のタイトルは `nav_translations` で対応。

### マニュアル用スクショ撮影の勘所（Windows / 2026-07-06 の実測値）
- **HDR モニターは画面キャプチャが真っ黒になる**（DXGI/GDI とも）。Win+Alt+B で HDR を一時オフにして撮る。
- **`GetWindowRect` は Win11 の影（シャドウ）余白を含む**。実枠は `DwmGetWindowAttribute(9)`
  （EXTENDED_FRAME_BOUNDS）。差分は環境依存（今回の実測: 左8/上0/右8/下8px）なので毎回実測してトリムする。
- **WebView2（Tauri）ウィンドウは `PrintWindow`（PW_RENDERFULLCONTENT）でも黒くなる**ことがある。
  画面座標からの `Graphics.CopyFromScreen` が確実（要 HDR オフ・要 Per-Monitor V2 DPI awareness。
  System-aware だとセカンダリモニターで座標が仮想化されてズレる）。
- **エクスポート進捗など一瞬の UI は連写で撮る**（300ms 間隔で 80〜150 枚 → 進捗バーの色で機械抽出）。
- PowerShell 5.1 は **BOM なし UTF-8 の .ps1 を Shift-JIS 誤読**してパースエラーになる。スクリプトは
  ASCII のみで書く（または BOM 付き UTF-8 で保存する）。

### GitHub Pages: Source を「GitHub Actions」にしないと deploy ジョブが 404/失敗
- **メモ**: `docs.yml` は actions/deploy-pages 方式。リポジトリの Settings → Pages → Build and deployment →
  Source を **GitHub Actions** にしておくこと（2026-07-06 設定済み）。gh-pages ブランチ方式ではない。
  トリガーは main push のうち `docs/**`・`mkdocs.yml`・`docs.yml` 変更時＋手動（workflow_dispatch）。

### Windows: 子プロセスのコンソール窓フラッシュ（dev では再現しない / 2026-07-08）
- **症状**: 配布版アプリで probe/preview/export など操作のたびにコンソール窓が一瞬出て消える。
- **原因**: リリースビルドは `windows_subsystem = "windows"` でコンソール非所持。console 系の
  子プロセス（ffmpeg/ffprobe/tmg1）起動ごとに自前の窓が作られる。`tauri dev` は `cargo run` 親の
  コンソールを子が継承するので**再現しない**（＝ dev で「出ない」は正常、判定材料にならない）。
- **回避策**: 子プロセスに `CREATE_NO_WINDOW`(`0x0800_0000`) を `creation_flags` で付与する。
  `ffmpeg.rs` の `command()` ヘルパー経由で全 `Command::new` を作ること（直書き禁止）。
- **教訓**: この種の「GUI アプリ特有」挙動は `cargo check`/`tauri dev` では観測不能。**必ず
  `npm run tauri build` のリリースビルド実機で確認する**。

## 地雷・禁止事項
- プレビューとエクスポートでフィルタチェーン構築を分岐させない（WYSIWYG が崩れる。
  修正は `filter.rs` に一本化する）。
- Windows で子プロセスを起動するときは生の `Command::new` を使わず `ffmpeg.rs` の `command()`
  ヘルパーを使う（コンソール窓フラッシュ防止。上記参照）。
- 表示文字列をハードコードしない（i18n。静的は `data-i18n`、動的は `t(key, params)`。言語追加は
  `locales/xx.json` + `i18n.ts` に1行）。
- TMG1 / コーデックのアルゴリズムを本リポジトリに持ち込まない（責務は `tmg1-codec` / `tmg1-cli`）。
