// システム PATH の ffmpeg / ffprobe を子プロセスとして叩くラッパ。
// 動画本体は webview には渡さず、パスだけを ffmpeg に渡してここで処理する
// （そのため fs/asset プラグイン権限は不要）。

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::filter::{build_chain, Chain, Segment};

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

/// 入力の [start, start+dur) を chain で monob raw 化して out に書き出す。
/// -ss(入力側=高速シーク) + -t(出力尺) で切り出し、-r で CFR 化して連結整合を取る。
fn slice_to_raw(
    input: &str,
    chain: &Chain,
    start: f64,
    dur: f64,
    fps: f64,
    out: &std::path::Path,
) -> Result<(), String> {
    let status = Command::new("ffmpeg")
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
    let mut child = Command::new("ffmpeg")
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
                let pct = if total_frames > 0 {
                    ((cum * 100) / total_frames).min(100) as i32
                } else {
                    0
                };
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

        slice_to_raw(&p.input_path, &chain, seg.start_sec, dur, p.fps, &segfile)
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

/// 再生範囲 [start, end) を区間ごとの設定で monob 化・連結し、ループ再生用の
/// mp4 バイト列を返す。フロントは Blob URL にして `<video loop>` で再生する。
pub fn render_range(app: &AppHandle, p: &Project, start: f64, end: f64) -> Result<Vec<u8>, String> {
    if p.width % 8 != 0 {
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
    let status = Command::new("ffmpeg")
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
