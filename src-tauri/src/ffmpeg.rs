// システム PATH の ffmpeg / ffprobe を子プロセスとして叩くラッパ。
// 動画本体は webview には渡さず、パスだけを ffmpeg に渡してここで処理する
// （そのため fs/asset プラグイン権限は不要）。

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

use crate::filter::{build_chain, Chain, Segment};

/// 子プロセス生成用の Command を作る。
/// Windows では CREATE_NO_WINDOW を付与し、操作のたびにコンソール窓が
/// 一瞬表示されて消える現象を防ぐ。他プラットフォームでは通常の Command::new と同じ。
#[allow(unused_mut)]
fn command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // winbase.h の CREATE_NO_WINDOW。子プロセスにコンソールを割り当てない。
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// 外部実行ファイル（ffmpeg / ffprobe / tmg1）のパス。
/// 設定で明示指定があればそれを、無ければ PATH 上のコマンド名を使う。
/// フロントが `settings.json`（tauri-plugin-store）へ書いた値を Rust 側で直読みする。
pub struct ExePaths {
    pub ffmpeg: String,
    pub ffprobe: String,
    pub tmg1: String,
}

impl Default for ExePaths {
    fn default() -> Self {
        // 既定は PATH 解決（従来どおりの挙動）。
        ExePaths {
            ffmpeg: "ffmpeg".to_string(),
            ffprobe: "ffprobe".to_string(),
            tmg1: "tmg1".to_string(),
        }
    }
}

impl ExePaths {
    /// `settings.json` から各実行パスを読む。キーが無い / 空文字なら既定（PATH）に戻す。
    /// ストアが開けない場合も既定にフォールバックする。
    pub fn load(app: &AppHandle) -> Self {
        let mut p = ExePaths::default();
        if let Ok(store) = app.store("settings.json") {
            let get = |key: &str| -> Option<String> {
                store
                    .get(key)
                    .and_then(|v| v.as_str().map(str::to_string))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            };
            if let Some(v) = get("ffmpegPath") {
                p.ffmpeg = v;
            }
            if let Some(v) = get("ffprobePath") {
                p.ffprobe = v;
            }
            if let Some(v) = get("tmg1Path") {
                p.tmg1 = v;
            }
        }
        p
    }
}

/// ffprobe で得た入力動画の情報。
#[derive(Debug, Serialize)]
pub struct VideoInfo {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
}

/// tmg1 エンコード設定（`tmg1 encode` のフラグに対応）。フロントから受け取る。
/// 各フィールドは CLI 既定に一致し、欠落時は `#[serde(default)]` で既定補完される
/// （v1 プロジェクトや encode を持たない呼び出しでも安全）。
/// `msb_first` / `invert` は monob 直接エンコード前提で固定のため露出しない。
#[derive(Debug, Clone, Deserialize)]
pub struct Tmg1Encode {
    /// エントロピー符号化器（"rice" / "range"）。
    #[serde(default = "default_coder")]
    pub coder: String,
    /// Rice パラメータ決定モード（"fixed" / "per-line" / "per-frame"）。
    #[serde(default = "default_rice_mode")]
    pub rice_mode: String,
    /// Fixed モードの Rice-k（0..7）。
    #[serde(default = "default_rice_k")]
    pub rice_k: u8,
    /// キーフレーム間隔。
    #[serde(default = "default_key_int")]
    pub key_int: u16,
    /// シーンチェンジ検出。
    #[serde(default = "default_true")]
    pub scd: bool,
    /// 可変フレームレート。
    #[serde(default = "default_true")]
    pub vfr: bool,
    /// 予測フィルタ。
    #[serde(default = "default_true")]
    pub prediction: bool,
    /// 差分（P）フレーム。
    #[serde(default = "default_true")]
    pub delta: bool,
    /// 末尾に TMGX 索引チャンクを付加。
    #[serde(default)]
    pub index: bool,
}

fn default_coder() -> String {
    "rice".to_string()
}
fn default_rice_mode() -> String {
    "per-line".to_string()
}
fn default_rice_k() -> u8 {
    1
}
fn default_key_int() -> u16 {
    60
}
fn default_true() -> bool {
    true
}

impl Default for Tmg1Encode {
    fn default() -> Self {
        Tmg1Encode {
            coder: default_coder(),
            rice_mode: default_rice_mode(),
            rice_k: default_rice_k(),
            key_int: default_key_int(),
            scd: true,
            vfr: true,
            prediction: true,
            delta: true,
            index: false,
        }
    }
}

/// エクスポート対象プロジェクト（フロントから受け取る）。
#[derive(Debug, Deserialize)]
pub struct Project {
    pub input_path: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub segments: Vec<Segment>,
    /// tmg1 エンコード設定（欠落時は既定）。
    #[serde(default)]
    pub encode: Tmg1Encode,
}

/// 出力形式。フロントの `<select id="out-format">` から受け取る。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// monob パック raw のみ。
    Raw,
    /// tmg1 のみ（raw は一時ファイルで内部利用のみ）。
    Tmg1,
    /// raw と tmg1 の両方。
    Both,
}

impl ExportFormat {
    fn wants_raw(self) -> bool {
        matches!(self, ExportFormat::Raw | ExportFormat::Both)
    }
    fn wants_tmg1(self) -> bool {
        matches!(self, ExportFormat::Tmg1 | ExportFormat::Both)
    }
}

/// エクスポート結果。
#[derive(Debug, Serialize, Clone)]
pub struct ExportResult {
    /// tmg1 encode に渡せる monob パック raw のパス（Raw/Both のときのみ）。
    pub raw_path: Option<String>,
    /// 直接再生可能な tmg1 のパス（Tmg1/Both のときのみ）。
    pub tmg1_path: Option<String>,
    /// 目視確認用に近傍拡大した mp4 のパス（preview 指定時のみ）。
    pub mp4_path: Option<String>,
    /// 総フレーム数。
    pub frames: u64,
    /// 未圧縮 monob（連結 raw）の総バイト数。レポートの圧縮率分母に使う。
    pub raw_bytes: u64,
    /// tmg1 出力ファイルのバイト数（Tmg1/Both のときのみ）。圧縮率分子に使う。
    pub tmg1_bytes: Option<u64>,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

#[derive(Clone, Serialize)]
struct RangeProgress {
    percent: u32,
}

/// `-r_frame_rate` 等の "30000/1001" 形式を f64 に変換。
fn parse_rational(s: &str) -> f64 {
    let mut it = s.split('/');
    let n: f64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let d: f64 = it.next().and_then(|x| x.parse().ok()).unwrap_or(1.0);
    if d == 0.0 {
        0.0
    } else {
        n / d
    }
}

/// ffprobe で入力動画のサイズ・fps・尺を取得する。
pub fn probe(exe: &ExePaths, path: &str) -> Result<VideoInfo, String> {
    let out = command(&exe.ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate:format=duration",
            "-of",
            "json",
            path,
        ])
        .output()
        .map_err(|e| format!("ffprobe を実行できませんでした: {e}. PATH に ffprobe はありますか？"))?;
    if !out.status.success() {
        return Err(format!(
            "ffprobe がエラーを返しました: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("ffprobe 出力の解析に失敗: {e}"))?;
    let stream = &v["streams"][0];
    let width = stream["width"].as_u64().unwrap_or(0) as u32;
    let height = stream["height"].as_u64().unwrap_or(0) as u32;
    let fps = parse_rational(stream["r_frame_rate"].as_str().unwrap_or("0/1"));
    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    if width == 0 || height == 0 {
        return Err("動画ストリームが見つかりませんでした".to_string());
    }
    Ok(VideoInfo {
        duration,
        fps,
        width,
        height,
    })
}

/// 指定時刻のフレームを、その区間のフィルタチェーンで monob 化し PNG バイト列で返す。
/// build_chain を通すのでプレビューはエクスポートと同一の絵になる。
pub fn render_preview(
    exe: &ExePaths,
    path: &str,
    time_sec: f64,
    seg: &Segment,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let chain = build_chain(seg, width, height);
    let out = command(&exe.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &time_sec.to_string(),
            "-i",
            path,
            "-vf",
            &chain.vf,
            "-frames:v",
            "1",
            "-pix_fmt",
            chain.pix_fmt,
            "-sws_dither",
            chain.sws_dither,
            "-c:v",
            "png",
            "-f",
            "image2",
            "pipe:1",
        ])
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("ffmpeg を実行できませんでした: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "ffmpeg がエラーを返しました: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

/// `out.raw` のような基準パスから `out.preview.mp4` を導く。
fn preview_mp4_path(raw_path: &Path) -> PathBuf {
    let stem = raw_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_string());
    let dir = raw_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!("{stem}.preview.mp4"))
}

/// 入力の [start, start+dur) を chain で monob raw 化して out に書き出す。
/// -ss(入力側=高速シーク) + -t(出力尺) で切り出し、-r で CFR 化して連結整合を取る。
fn slice_to_raw(
    exe: &ExePaths,
    input: &str,
    chain: &Chain,
    start: f64,
    dur: f64,
    fps: f64,
    out: &std::path::Path,
) -> Result<(), String> {
    let status = command(&exe.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &start.to_string(),
            "-i",
            input,
            "-t",
            &dur.to_string(),
            "-vf",
            &chain.vf,
            "-r",
            &fps.to_string(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            chain.pix_fmt,
            "-sws_dither",
            chain.sws_dither,
        ])
        .arg(out)
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("ffmpeg を実行できませんでした: {e}"))?;
    if !status.status.success() {
        return Err(String::from_utf8_lossy(&status.stderr).into_owned());
    }
    Ok(())
}

/// slice_to_raw と同じだが、ffmpeg の `-progress pipe:1` を解析して
/// 範囲全体（total_frames）に対する累積フレームから 0–100% を emit する。
/// frames_before はこのスライス開始前までの累積フレーム数。
#[allow(clippy::too_many_arguments)]
fn slice_to_raw_progress(
    app: &AppHandle,
    exe: &ExePaths,
    input: &str,
    chain: &Chain,
    start: f64,
    dur: f64,
    fps: f64,
    out: &Path,
    frames_before: u64,
    total_frames: u64,
    last_pct: &mut i32,
) -> Result<(), String> {
    let mut child = command(&exe.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &start.to_string(),
            "-i",
            input,
            "-t",
            &dur.to_string(),
            "-vf",
            &chain.vf,
            "-r",
            &fps.to_string(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            chain.pix_fmt,
            "-sws_dither",
            chain.sws_dither,
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .arg(out)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg を実行できませんでした: {e}"))?;

    let stdout = child.stdout.take().ok_or("stdout を取得できません")?;
    // stderr はデッドロック防止のため別スレッドで吸い出す（失敗時のエラー本文用）。
    let mut stderr = child.stderr.take().ok_or("stderr を取得できません")?;
    let err_handle = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    // `-progress` は key=value 行を出力する。frame= の値で進捗を算出。
    for line in BufReader::new(stdout).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if let Some(rest) = line.strip_prefix("frame=") {
            if let Ok(f) = rest.trim().parse::<u64>() {
                let cum = frames_before + f;
                let pct = (cum * 100).checked_div(total_frames).map_or(0, |v| v.min(100)) as i32;
                if pct != *last_pct {
                    *last_pct = pct;
                    let _ = app.emit("range-progress", RangeProgress { percent: pct as u32 });
                }
            }
        }
    }

    let status = child.wait().map_err(|e| format!("ffmpeg 待機失敗: {e}"))?;
    let errtext = err_handle.join().unwrap_or_default();
    if !status.success() {
        return Err(errtext);
    }
    Ok(())
}

/// monob パック raw を PATH 上の `tmg1` CLI で tmg1 にエンコードする。
/// codec 本体は本リポジトリに持ち込まず、ffmpeg 同様にサブプロセスへ委ねる。
/// Studio の monob は MSB-first で CLI の既定と整合するため追加変換は不要。
fn encode_tmg1(
    exe: &ExePaths,
    raw_path: &Path,
    width: u32,
    height: u32,
    fps: f64,
    enc: &Tmg1Encode,
    out_tmg1: &Path,
) -> Result<(), String> {
    let size = format!("{width}x{height}");
    // --fps は u16。Studio の fps は f64 なので四捨五入して整数で渡す。
    let fps = (fps.round() as u16).to_string();
    let rice_k = enc.rice_k.to_string();
    let key_int = enc.key_int.to_string();
    // bool フラグは CLI 側が値付き（--flag true/false）を要求する。
    let b = |v: bool| if v { "true" } else { "false" };

    let mut cmd = command(&exe.tmg1);
    cmd.args(["encode", "--size", &size, "--fps", &fps]);
    cmd.args(["--coder", &enc.coder]);
    cmd.args(["--key-int", &key_int]);
    cmd.args(["--rice-mode", &enc.rice_mode]);
    cmd.args(["--rice-k", &rice_k]);
    cmd.args(["--scd", b(enc.scd)]);
    cmd.args(["--vfr", b(enc.vfr)]);
    cmd.args(["--prediction", b(enc.prediction)]);
    cmd.args(["--delta", b(enc.delta)]);
    // --index は真偽フラグ（値を付けるとエラー）。真のときのみ付与。
    if enc.index {
        cmd.arg("--index");
    }
    // msb-first / invert は monob 前提で固定のため渡さない（CLI 既定 true/false のまま）。
    cmd.arg("-i")
        .arg(raw_path)
        .arg("-o")
        .arg(out_tmg1)
        .stderr(Stdio::piped());
    let out = cmd
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "tmg1 CLI が見つかりません。PATH に通すか、設定で tmg1 の実行パスを指定してください".to_string()
            } else {
                format!("tmg1 encode を実行できませんでした: {e}")
            }
        })?;
    if !out.status.success() {
        return Err(format!(
            "tmg1 エンコードに失敗: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// 全区間を個別に monob raw 化 → 連結し、指定形式（raw / tmg1 / 両方）で出力する。
/// preview が true のときのみ目視用 mp4 も生成する。
pub fn export(
    app: &AppHandle,
    exe: &ExePaths,
    p: &Project,
    out_path: &str,
    format: ExportFormat,
    preview: bool,
) -> Result<ExportResult, String> {
    if !p.width.is_multiple_of(8) {
        return Err(format!(
            "幅 {} は 8 の倍数にしてください（monob のバイト境界のため）",
            p.width
        ));
    }
    if p.segments.is_empty() {
        return Err("区間がありません".to_string());
    }

    let tmp = std::env::temp_dir().join(format!("tmg1studio_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("一時フォルダ作成失敗: {e}"))?;

    // 出力ファイル名は out_path の拡張子を除いた「dir + stem」を基準に raw/tmg1/preview.mp4 を導出する。
    // これにより形式選択と実際に書く拡張子が食い違わないようにする。
    let base = {
        let pb = PathBuf::from(out_path);
        let stem = pb
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "output".to_string());
        let dir = pb.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        dir.join(stem)
    };
    let raw_out = base.with_extension("raw");
    let tmg1_out = base.with_extension("tmg1");

    // raw を書き出す先。raw を成果物にしない（tmg1 のみ）ときは一時ファイルへ。
    let raw_write_path = if format.wants_raw() {
        raw_out.clone()
    } else {
        tmp.join("concat.raw")
    };

    let frame_bytes = (p.width as u64 * p.height as u64) / 8;
    let total = p.segments.len();

    // 出力ファイルへ各区間 raw を順次追記（全体をメモリに載せない）。
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(&raw_write_path).map_err(|e| format!("出力ファイル作成失敗: {e}"))?,
    );
    let mut total_bytes: u64 = 0;

    for (i, seg) in p.segments.iter().enumerate() {
        let chain = build_chain(seg, p.width, p.height);
        let dur = (seg.end_sec - seg.start_sec).max(0.0);
        let segfile = tmp.join(format!("seg{i}.raw"));

        slice_to_raw(exe, &p.input_path, &chain, seg.start_sec, dur, p.fps, &segfile)
            .map_err(|e| format!("区間 {} のエンコードに失敗: {e}", i + 1))?;

        let bytes = std::fs::read(&segfile).map_err(|e| format!("区間 raw 読み込み失敗: {e}"))?;
        total_bytes += bytes.len() as u64;
        writer
            .write_all(&bytes)
            .map_err(|e| format!("raw 追記失敗: {e}"))?;
        let _ = std::fs::remove_file(&segfile);

        let _ = app.emit("export-progress", Progress { done: i + 1, total });
    }
    writer.flush().map_err(|e| format!("raw フラッシュ失敗: {e}"))?;
    drop(writer);

    // 目視確認用 mp4（近傍拡大 6x, yuv420p）。preview 指定時のみ生成する。
    let mp4_path = if preview {
        let mp4_path = preview_mp4_path(&raw_out);
        let size = format!("{}x{}", p.width, p.height);
        let mp4_status = command(&exe.ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "monob",
                "-s",
                &size,
                "-r",
                &p.fps.to_string(),
                "-i",
            ])
            .arg(&raw_write_path)
            .args([
                "-vf",
                "scale=iw*6:ih*6:flags=neighbor",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&mp4_path)
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("プレビュー mp4 生成の ffmpeg 実行失敗: {e}"))?;
        if !mp4_status.status.success() {
            return Err(format!(
                "プレビュー mp4 の生成に失敗: {}",
                String::from_utf8_lossy(&mp4_status.stderr)
            ));
        }
        Some(mp4_path.to_string_lossy().into_owned())
    } else {
        None
    };

    // tmg1 化（形式に含まれるときのみ）。連結済み raw を PATH の tmg1 CLI に委ねる。
    // 成功後に出力ファイルサイズを取得し、レポートの圧縮率算出に使う。
    let tmg1_bytes = if format.wants_tmg1() {
        let _ = app.emit("tmg1-encoding", ());
        encode_tmg1(exe, &raw_write_path, p.width, p.height, p.fps, &p.encode, &tmg1_out)?;
        std::fs::metadata(&tmg1_out).map(|m| m.len()).ok()
    } else {
        None
    };

    // tmg1 のみのときは一時 raw を掃除する。
    if !format.wants_raw() {
        let _ = std::fs::remove_file(&raw_write_path);
    }

    let frames = total_bytes.checked_div(frame_bytes).unwrap_or(0);

    Ok(ExportResult {
        raw_path: format.wants_raw().then(|| raw_out.to_string_lossy().into_owned()),
        tmg1_path: format.wants_tmg1().then(|| tmg1_out.to_string_lossy().into_owned()),
        mp4_path,
        frames,
        raw_bytes: total_bytes,
        tmg1_bytes,
    })
}

/// 再生範囲 [start, end) を区間ごとの設定で monob 化・連結し、ループ再生用の
/// mp4 バイト列を返す。フロントは Blob URL にして `<video loop>` で再生する。
pub fn render_range(
    app: &AppHandle,
    exe: &ExePaths,
    p: &Project,
    start: f64,
    end: f64,
) -> Result<Vec<u8>, String> {
    if !p.width.is_multiple_of(8) {
        return Err(format!(
            "幅 {} は 8 の倍数にしてください（monob のバイト境界のため）",
            p.width
        ));
    }
    if end <= start {
        return Err("再生範囲が空です".to_string());
    }

    // 進捗の分母: 範囲に重なる各スライスの推定フレーム数の合計。
    let total_frames: u64 = p
        .segments
        .iter()
        .filter_map(|seg| {
            let s = seg.start_sec.max(start);
            let e = seg.end_sec.min(end);
            if e <= s + 1e-6 {
                None
            } else {
                Some(((e - s) * p.fps).round() as u64)
            }
        })
        .sum();

    // 同時実行やエクスポートと衝突しないよう、呼び出しごとに一意な一時フォルダを使う。
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("tmg1studio_range_{}_{}", std::process::id(), stamp));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("一時フォルダ作成失敗: {e}"))?;

    let raw_path = tmp.join("range.raw");
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(&raw_path).map_err(|e| format!("一時 raw 作成失敗: {e}"))?,
    );
    let mut total_bytes: u64 = 0;
    let mut frames_before: u64 = 0;
    let mut last_pct: i32 = -1;

    // 範囲に重なる区間を、区間ごとの chain でクランプしたスライスだけレンダリングして連結。
    for (i, seg) in p.segments.iter().enumerate() {
        let s = seg.start_sec.max(start);
        let e = seg.end_sec.min(end);
        if e <= s + 1e-6 {
            continue;
        }
        let chain = build_chain(seg, p.width, p.height);
        let segfile = tmp.join(format!("s{i}.raw"));
        slice_to_raw_progress(
            app,
            exe,
            &p.input_path,
            &chain,
            s,
            e - s,
            p.fps,
            &segfile,
            frames_before,
            total_frames,
            &mut last_pct,
        )
        .map_err(|err| format!("再生範囲のレンダリングに失敗: {err}"))?;
        frames_before += ((e - s) * p.fps).round() as u64;
        let bytes = std::fs::read(&segfile).map_err(|err| format!("range raw 読み込み失敗: {err}"))?;
        total_bytes += bytes.len() as u64;
        writer
            .write_all(&bytes)
            .map_err(|err| format!("range raw 追記失敗: {err}"))?;
        let _ = std::fs::remove_file(&segfile);
    }
    writer.flush().map_err(|e| format!("range raw フラッシュ失敗: {e}"))?;
    drop(writer);
    // デコード完了（mp4 化は短時間）。バーを 100% にしておく。
    let _ = app.emit("range-progress", RangeProgress { percent: 100 });

    if total_bytes == 0 {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("再生範囲に有効なフレームがありません".to_string());
    }

    // 近傍拡大 mp4（yuv420p）。倍率 k は幅が ~480px 前後になるよう決め、偶数に丸める。
    let k = ((480.0 / p.width as f64).round() as u32).max(1);
    let mut vw = p.width * k;
    let mut vh = p.height * k;
    if vw % 2 == 1 {
        vw += 1;
    }
    if vh % 2 == 1 {
        vh += 1;
    }
    let mp4 = tmp.join("range.mp4");
    let size = format!("{}x{}", p.width, p.height);
    let vf = format!("scale={vw}:{vh}:flags=neighbor");
    let status = command(&exe.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "monob",
            "-s",
            &size,
            "-r",
            &p.fps.to_string(),
            "-i",
        ])
        .arg(&raw_path)
        .args(["-vf", &vf, "-pix_fmt", "yuv420p", "-an"])
        .arg(&mp4)
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("再生用 mp4 生成の ffmpeg 実行失敗: {e}"))?;
    if !status.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!(
            "再生用 mp4 の生成に失敗: {}",
            String::from_utf8_lossy(&status.stderr)
        ));
    }

    let data = std::fs::read(&mp4).map_err(|e| format!("再生用 mp4 読み込み失敗: {e}"))?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(data)
}
