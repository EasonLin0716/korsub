use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::ffmpeg::resolve_bin;
use crate::types::{ProgressEvent, SubtitleItem};

/// 韓文辨識品質與速度的平衡選擇（約 1.6GB）
pub const MODEL_FILE: &str = "ggml-large-v3-turbo.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin";

pub fn resolve_whisper() -> Option<PathBuf> {
    resolve_bin("whisper-cli").or_else(|| resolve_bin("whisper-cpp"))
}

pub fn model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("無法取得資料目錄: {e}"))?;
    Ok(dir.join("models").join(MODEL_FILE))
}

fn emit(app: &AppHandle, stage: &str, percent: f64, message: String) {
    let _ = app.emit(
        "pipeline-progress",
        ProgressEvent {
            stage: stage.into(),
            percent,
            message,
        },
    );
}

/// 從 Hugging Face 下載 whisper 模型檔到 app 資料目錄，串流寫入並回報進度
pub async fn download_model(app: &AppHandle) -> Result<(), String> {
    let target = model_path(app)?;
    if target.exists() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("建立模型目錄失敗: {e}"))?;
    }

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(MODEL_URL)
        .send()
        .await
        .map_err(|e| format!("模型下載連線失敗: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("模型下載失敗 (HTTP {})", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let tmp = target.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("建立模型暫存檔失敗: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut last_emitted_percent = -1.0_f64;
    let mut resp = resp;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("模型下載中斷: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("寫入模型檔失敗: {e}"))?;
        downloaded += chunk.len() as u64;
        if total > 0 {
            let percent = downloaded as f64 / total as f64 * 100.0;
            if percent - last_emitted_percent >= 1.0 {
                last_emitted_percent = percent;
                emit(
                    app,
                    "model",
                    percent,
                    format!(
                        "下載辨識模型中… {:.0}/{:.0} MB",
                        downloaded as f64 / 1_048_576.0,
                        total as f64 / 1_048_576.0
                    ),
                );
            }
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    tokio::fs::rename(&tmp, &target)
        .await
        .map_err(|e| format!("模型檔搬移失敗: {e}"))?;
    emit(app, "model", 100.0, "模型下載完成".into());
    Ok(())
}

#[derive(Deserialize)]
struct WhisperJson {
    transcription: Vec<WhisperSegment>,
}

#[derive(Deserialize)]
struct WhisperSegment {
    offsets: WhisperOffsets,
    text: String,
}

#[derive(Deserialize)]
struct WhisperOffsets {
    /// 毫秒
    from: u64,
    to: u64,
}

/// 以本機 whisper.cpp 辨識 16kHz WAV，回傳帶時間戳的字幕條目
pub async fn transcribe(
    app: &AppHandle,
    audio: &Path,
    model: &Path,
) -> Result<Vec<SubtitleItem>, String> {
    let bin = resolve_whisper().ok_or(
        "找不到 whisper.cpp，請先安裝（macOS: brew install whisper-cpp）".to_string(),
    )?;

    let out_prefix = std::env::temp_dir().join("korsub_transcript");
    let json_path = out_prefix.with_extension("json");
    let _ = tokio::fs::remove_file(&json_path).await;

    let mut child = Command::new(bin)
        .arg("-m")
        .arg(model)
        .args(["-l", "ko", "-oj", "-pp", "-np"])
        .arg("-of")
        .arg(&out_prefix)
        .arg(audio)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("執行 whisper-cli 失敗: {e}"))?;

    let mut stderr_tail: Vec<String> = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // -pp 會印出 "whisper_print_progress_callback: progress = 5%"
            if let Some(pos) = line.find("progress =") {
                let value = line[pos + "progress =".len()..].trim().trim_end_matches('%');
                if let Ok(percent) = value.trim().parse::<f64>() {
                    emit(
                        app,
                        "transcribe",
                        percent,
                        format!("本機語音辨識中… {percent:.0}%"),
                    );
                }
            } else {
                stderr_tail.push(line);
                if stderr_tail.len() > 20 {
                    stderr_tail.remove(0);
                }
            }
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("語音辨識失敗:\n{}", stderr_tail.join("\n")));
    }

    // whisper.cpp 的 token 可能在多位元組字元中間切斷，JSON 檔會夾帶不合法的
    // UTF-8 位元組，需以 lossy 方式讀取再清掉替換字元
    let raw_bytes = tokio::fs::read(&json_path)
        .await
        .map_err(|e| format!("讀取辨識結果失敗: {e}"))?;
    let raw = String::from_utf8_lossy(&raw_bytes).into_owned();
    let _ = tokio::fs::remove_file(&json_path).await;

    let parsed: WhisperJson =
        serde_json::from_str(&raw).map_err(|e| format!("辨識結果解析失敗: {e}"))?;

    Ok(parsed
        .transcription
        .into_iter()
        .filter_map(|s| {
            let ko = s.text.replace('\u{FFFD}', "").trim().to_string();
            if ko.is_empty() {
                return None;
            }
            Some((s.offsets, ko))
        })
        .enumerate()
        .map(|(i, (offsets, ko))| SubtitleItem {
            index: i,
            start: offsets.from as f64 / 1000.0,
            end: offsets.to as f64 / 1000.0,
            ko,
            zh: String::new(),
        })
        .collect())
}
