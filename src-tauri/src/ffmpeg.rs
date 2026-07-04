// システム PATH の ffmpeg / ffprobe を子プロセスとして叩くラッパ。
// 動画本体は webview には渡さず、パスだけを ffmpeg に渡してここで処理する
// （そのため fs/asset プラグイン権限は不要）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::filter::{build_chain, Segment};

/// ffprobe で得た入力動画の情報。
#[derive(Debug, Serialize)]
pub struct VideoInfo {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
}

/// エクスポート対象プロジェクト（フロントから受け取る）。
#[derive(Debug, Deserialize)]
pub struct Project {
    pub input_path: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub segments: Vec<Segment>,
}

/// エクスポート結果。
#[derive(Debug, Serialize, Clone)]
pub struct ExportResult {
    /// tmg1 encode に渡せる monob パック raw のパス。
    pub raw_path: String,
    /// 目視確認用に等倍近傍拡大した mp4 のパス。
    pub mp4_path: String,
    /// 総フレーム数。
    pub frames: u64,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
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
pub fn probe(path: &str) -> Result<VideoInfo, String> {
    let out = Command::new("ffprobe")
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
    path: &str,
    time_sec: f64,
    seg: &Segment,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let chain = build_chain(seg, width, height);
    let out = Command::new("ffmpeg")
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

/// 全区間を個別に monob raw 化 → 連結 → 目視用 mp4 を生成する。
pub fn export(app: &AppHandle, p: &Project, out_path: &str) -> Result<ExportResult, String> {
    if p.width % 8 != 0 {
        return Err(format!(
            "幅 {} は 8 の倍数にしてください（monob のバイト境界のため）",
            p.width
        ));
    }
    if p.segments.is_empty() {
        return Err("区間がありません".to_string());
    }

    // raw 出力パス（拡張子が無ければ .raw を付ける）。
    let raw_path = {
        let pb = PathBuf::from(out_path);
        if pb.extension().is_some() {
            pb
        } else {
            pb.with_extension("raw")
        }
    };

    let tmp = std::env::temp_dir().join(format!("tmg1studio_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("一時フォルダ作成失敗: {e}"))?;

    let frame_bytes = (p.width as u64 * p.height as u64) / 8;
    let total = p.segments.len();

    // 出力ファイルへ各区間 raw を順次追記（全体をメモリに載せない）。
    let mut writer = std::io::BufWriter::new(
        std::fs::File::create(&raw_path).map_err(|e| format!("出力ファイル作成失敗: {e}"))?,
    );
    let mut total_bytes: u64 = 0;

    for (i, seg) in p.segments.iter().enumerate() {
        let chain = build_chain(seg, p.width, p.height);
        let dur = (seg.end_sec - seg.start_sec).max(0.0);
        let segfile = tmp.join(format!("seg{i}.raw"));

        // -ss(入力側=高速シーク) + -t(出力尺) で区間を切り出し、-r で CFR 化して連結整合を取る。
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-ss",
                &seg.start_sec.to_string(),
                "-i",
                &p.input_path,
                "-t",
                &dur.to_string(),
                "-vf",
                &chain.vf,
                "-r",
                &p.fps.to_string(),
                "-f",
                "rawvideo",
                "-pix_fmt",
                chain.pix_fmt,
                "-sws_dither",
                chain.sws_dither,
            ])
            .arg(&segfile)
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("ffmpeg を実行できませんでした: {e}"))?;
        if !status.status.success() {
            return Err(format!(
                "区間 {} のエンコードに失敗: {}",
                i + 1,
                String::from_utf8_lossy(&status.stderr)
            ));
        }

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

    // 目視確認用 mp4（近傍拡大 6x, yuv420p）。
    let mp4_path = preview_mp4_path(&raw_path);
    let size = format!("{}x{}", p.width, p.height);
    let mp4_status = Command::new("ffmpeg")
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

    let frames = if frame_bytes > 0 {
        total_bytes / frame_bytes
    } else {
        0
    };

    Ok(ExportResult {
        raw_path: raw_path.to_string_lossy().into_owned(),
        mp4_path: mp4_path.to_string_lossy().into_owned(),
        frames,
    })
}
