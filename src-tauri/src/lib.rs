mod ffmpeg;
mod settings;
mod srt;
mod translate;
mod types;
mod whisper;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use types::{ProgressEvent, Settings, SubtitleItem};

#[derive(Serialize)]
struct EnvStatus {
    ffmpeg: Option<String>,
    ffprobe: Option<String>,
    whisper: Option<String>,
    model_downloaded: bool,
}

#[tauri::command]
fn check_environment(app: AppHandle) -> EnvStatus {
    let model_downloaded = whisper::model_path(&app)
        .map(|p| p.exists())
        .unwrap_or(false);
    EnvStatus {
        ffmpeg: ffmpeg::resolve_bin("ffmpeg").map(|p| p.to_string_lossy().into_owned()),
        ffprobe: ffmpeg::resolve_bin("ffprobe").map(|p| p.to_string_lossy().into_owned()),
        whisper: whisper::resolve_whisper().map(|p| p.to_string_lossy().into_owned()),
        model_downloaded,
    }
}

/// 下載 whisper.cpp 辨識模型（約 1.6GB，只需一次）
#[tauri::command]
async fn download_model(app: AppHandle) -> Result<(), String> {
    whisper::download_model(&app).await
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<Settings, String> {
    settings::load(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&app, &settings)
}

fn emit_progress(app: &AppHandle, stage: &str, percent: f64, message: impl Into<String>) {
    let _ = app.emit(
        "pipeline-progress",
        ProgressEvent {
            stage: stage.into(),
            percent,
            message: message.into(),
        },
    );
}

/// 抽出音軌並以本機 whisper.cpp 辨識，回傳含時間戳的韓文字幕
#[tauri::command]
async fn transcribe_video(app: AppHandle, video_path: String) -> Result<Vec<SubtitleItem>, String> {
    let model = whisper::model_path(&app)?;
    if !model.exists() {
        return Err("辨識模型尚未下載，請先點「下載辨識模型」".to_string());
    }

    let audio_path = std::env::temp_dir().join("korsub_audio.wav");

    emit_progress(&app, "extract", -1.0, "抽取音軌中…");
    ffmpeg::extract_audio(&video_path, &audio_path).await?;

    emit_progress(&app, "transcribe", 0.0, "本機語音辨識中…");
    let result = whisper::transcribe(&app, &audio_path, &model).await;

    let _ = tokio::fs::remove_file(&audio_path).await;

    let subtitles = result?;
    emit_progress(
        &app,
        "transcribe",
        100.0,
        format!("辨識完成，共 {} 條字幕", subtitles.len()),
    );
    Ok(subtitles)
}

/// 以 Claude API 將韓文字幕翻譯成繁體中文
#[tauri::command]
async fn translate_subtitles(
    app: AppHandle,
    subtitles: Vec<SubtitleItem>,
) -> Result<Vec<SubtitleItem>, String> {
    let cfg = settings::load(&app)?;
    if cfg.anthropic_api_key.trim().is_empty() {
        return Err("尚未設定 Anthropic API Key，請先到設定填入".to_string());
    }
    translate::translate_all(&app, cfg.anthropic_api_key.trim(), subtitles).await
}

/// 匯出 SRT 軟字幕檔
#[tauri::command]
fn export_srt(subtitles: Vec<SubtitleItem>, path: String, bilingual: bool) -> Result<(), String> {
    let content = srt::build(&subtitles, bilingual);
    std::fs::write(&path, content).map_err(|e| format!("寫入 SRT 失敗: {e}"))
}

/// 匯入既有 SRT 字幕，跳過辨識與翻譯
#[tauri::command]
fn import_srt(path: String) -> Result<Vec<SubtitleItem>, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("讀取 SRT 失敗: {e}"))?;
    let content = String::from_utf8_lossy(&bytes);
    let items = srt::parse(&content);
    if items.is_empty() {
        return Err("無法從檔案解析出任何字幕，請確認是有效的 SRT 檔".to_string());
    }
    Ok(items)
}

/// 燒錄硬字幕（重新編碼輸出新影片）
#[tauri::command]
async fn burn_subtitles(
    app: AppHandle,
    video_path: String,
    subtitles: Vec<SubtitleItem>,
    output_path: String,
    bilingual: bool,
) -> Result<(), String> {
    let srt_content = srt::build(&subtitles, bilingual);
    let srt_path = std::env::temp_dir().join("korsub_burn.srt");
    tokio::fs::write(&srt_path, srt_content)
        .await
        .map_err(|e| format!("寫入暫存字幕失敗: {e}"))?;

    emit_progress(&app, "burn", 0.0, "開始燒錄硬字幕…");
    let result = ffmpeg::burn_subtitles(&app, &video_path, &srt_path, &output_path).await;
    let _ = tokio::fs::remove_file(&srt_path).await;
    result?;

    emit_progress(&app, "burn", 100.0, "硬字幕輸出完成");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            check_environment,
            download_model,
            load_settings,
            save_settings,
            transcribe_video,
            translate_subtitles,
            export_srt,
            import_srt,
            burn_subtitles
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
