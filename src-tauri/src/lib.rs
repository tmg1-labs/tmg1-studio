mod ffmpeg;
mod filter;

use base64::{engine::general_purpose::STANDARD, Engine};

use ffmpeg::{ExportResult, Project, VideoInfo};
use filter::Segment;

/// 入力動画の情報を取得する。
#[tauri::command]
async fn probe_video(path: String) -> Result<VideoInfo, String> {
    tauri::async_runtime::spawn_blocking(move || ffmpeg::probe(&path))
        .await
        .map_err(|e| format!("タスク実行失敗: {e}"))?
}

/// 指定時刻のプレビューフレームを PNG data URL 文字列で返す。
#[tauri::command]
async fn render_preview(
    path: String,
    time_sec: f64,
    segment: Segment,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let png = tauri::async_runtime::spawn_blocking(move || {
        ffmpeg::render_preview(&path, time_sec, &segment, width, height)
    })
    .await
    .map_err(|e| format!("タスク実行失敗: {e}"))??;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&png)))
}

/// 再生範囲をレンダリングし、ループ再生用 mp4 を ArrayBuffer（生バイト）で返す。
#[tauri::command]
async fn render_range(
    app: tauri::AppHandle,
    project: Project,
    start_sec: f64,
    end_sec: f64,
) -> Result<tauri::ipc::Response, String> {
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        ffmpeg::render_range(&app, &project, start_sec, end_sec)
    })
    .await
    .map_err(|e| format!("タスク実行失敗: {e}"))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// 編集プロジェクト（JSON 文字列）をファイルに保存する。
#[tauri::command]
fn save_project(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("プロジェクト保存に失敗: {e}"))
}

/// 編集プロジェクト（JSON 文字列）をファイルから読み込む。
#[tauri::command]
fn load_project(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("プロジェクト読み込みに失敗: {e}"))
}

/// プロジェクトをエクスポート（monob raw + 目視用 mp4）。
#[tauri::command]
async fn export(
    app: tauri::AppHandle,
    project: Project,
    out_path: String,
) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || ffmpeg::export(&app, &project, &out_path))
        .await
        .map_err(|e| format!("タスク実行失敗: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            probe_video,
            render_preview,
            render_range,
            export,
            save_project,
            load_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
