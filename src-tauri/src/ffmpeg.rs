use std::path::{Path, PathBuf};
use std::process::Stdio;

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::types::ProgressEvent;

/// GUI app 從 Finder 啟動時 PATH 很短，補上常見安裝位置。
/// 優先找 keg-only 的 ffmpeg-full（Homebrew 核心的 ffmpeg 8 已拿掉 libass，
/// 沒有 subtitles filter，燒錄硬字幕需要 full build）
pub fn resolve_bin(name: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in [
        "/opt/homebrew/opt/ffmpeg-full/bin",
        "/usr/local/opt/ffmpeg-full/bin",
    ] {
        candidates.push(Path::new(dir).join(name));
    }
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_env) {
            candidates.push(dir.join(name));
        }
    }
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        candidates.push(Path::new(dir).join(name));
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn require_bin(name: &str) -> Result<PathBuf, String> {
    resolve_bin(name).ok_or_else(|| {
        format!("找不到 {name}，請先安裝 FFmpeg（macOS: brew install ffmpeg）")
    })
}

/// 從影片抽出 16kHz 單聲道 WAV（whisper.cpp 的標準輸入格式）
pub async fn extract_audio(video: &str, out: &Path) -> Result<(), String> {
    let ffmpeg = require_bin("ffmpeg")?;
    let output = Command::new(ffmpeg)
        .args(["-y", "-i", video, "-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(out)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("執行 ffmpeg 失敗: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: String = stderr.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(format!("音訊抽取失敗:\n{tail}"));
    }
    Ok(())
}

/// 取得影片長度（秒）
pub async fn probe_duration(video: &str) -> Result<f64, String> {
    let ffprobe = require_bin("ffprobe")?;
    let output = Command::new(ffprobe)
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "csv=p=0",
            video,
        ])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("執行 ffprobe 失敗: {e}"))?;
    if !output.status.success() {
        return Err("無法讀取影片資訊，請確認檔案格式".to_string());
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|_| "無法解析影片長度".to_string())
}

/// subtitles filter 的檔名需跳脫 `\` `'` `:`
fn escape_filter_path(path: &str) -> String {
    path.replace('\\', "\\\\").replace('\'', "\\'").replace(':', "\\:")
}

/// 用 ffmpeg 把字幕燒錄進畫面（重新編碼），解析 -progress 輸出回報進度
pub async fn burn_subtitles(
    app: &AppHandle,
    video: &str,
    srt_path: &Path,
    output_path: &str,
) -> Result<(), String> {
    let ffmpeg = require_bin("ffmpeg")?;
    let duration = probe_duration(video).await.unwrap_or(0.0);

    // 參數經 exec 直接傳給 ffmpeg（無 shell），值不需要再加引號
    let vf = format!(
        "subtitles={}:force_style=FontSize=18",
        escape_filter_path(&srt_path.to_string_lossy())
    );

    let mut child = Command::new(ffmpeg)
        .args(["-y", "-i", video, "-vf", &vf, "-c:a", "copy"])
        .args(["-progress", "pipe:1", "-nostats", "-loglevel", "error"])
        .arg(output_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("執行 ffmpeg 失敗: {e}"))?;

    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(value) = line.strip_prefix("out_time_ms=") {
                if duration > 0.0 {
                    if let Ok(us) = value.trim().parse::<f64>() {
                        // out_time_ms 實際單位是微秒
                        let percent = ((us / 1_000_000.0) / duration * 100.0).clamp(0.0, 100.0);
                        let _ = app.emit(
                            "pipeline-progress",
                            ProgressEvent {
                                stage: "burn".into(),
                                percent,
                                message: format!("燒錄硬字幕中… {percent:.0}%"),
                            },
                        );
                    }
                }
            }
        }
    }

    let mut stderr_text = String::new();
    if let Some(stderr) = child.stderr.take() {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            stderr_text.push_str(&line);
            stderr_text.push('\n');
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("硬字幕燒錄失敗:\n{}", stderr_text.trim()));
    }
    Ok(())
}
