use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::types::{ProgressEvent, SubtitleItem};

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-opus-4-8";
/// 每次請求翻譯的字幕條數
const CHUNK_SIZE: usize = 25;
/// 帶入前一段結尾作為上下文的行數
const CONTEXT_LINES: usize = 3;

const SYSTEM_PROMPT: &str = "你是專業的影視字幕翻譯，將韓文字幕逐條翻譯成台灣慣用的繁體中文。\
要求：語感自然口語、符合台灣用語習慣；一條對一條，不可合併或拆分；\
保留語氣詞的情緒但不逐字直譯；人名採常見中文譯名或保留原文；\
只輸出翻譯結果，不要任何解釋。";

fn output_schema() -> Value {
    json!({
        "type": "json_schema",
        "schema": {
            "type": "object",
            "properties": {
                "translations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "index": { "type": "integer" },
                            "text": { "type": "string" }
                        },
                        "required": ["index", "text"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["translations"],
            "additionalProperties": false
        }
    })
}

fn build_user_message(chunk: &[SubtitleItem], context: &[String]) -> String {
    let mut msg = String::new();
    if !context.is_empty() {
        msg.push_str("（前文脈絡，僅供參考，不需翻譯）\n");
        for line in context {
            msg.push_str(line);
            msg.push('\n');
        }
        msg.push('\n');
    }
    msg.push_str("請翻譯以下韓文字幕，依編號回傳：\n");
    for item in chunk {
        msg.push_str(&format!("{}. {}\n", item.index, item.ko));
    }
    msg
}

async fn translate_chunk(
    client: &reqwest::Client,
    api_key: &str,
    chunk: &[SubtitleItem],
    context: &[String],
) -> Result<HashMap<usize, String>, String> {
    let body = json!({
        "model": MODEL,
        "max_tokens": 16000,
        "thinking": { "type": "adaptive" },
        "system": SYSTEM_PROMPT,
        "output_config": { "format": output_schema() },
        "messages": [
            { "role": "user", "content": build_user_message(chunk, context) }
        ]
    });

    let resp = client
        .post(ANTHROPIC_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Claude API 連線失敗: {e}"))?;

    let status = resp.status();
    let body_text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Claude API 錯誤 ({status}): {body_text}"));
    }

    let parsed: Value =
        serde_json::from_str(&body_text).map_err(|e| format!("Claude 回應解析失敗: {e}"))?;

    if parsed["stop_reason"] == "refusal" {
        return Err("Claude 拒絕了這段翻譯請求，請檢查內容後重試".to_string());
    }

    // 結構化輸出保證有 text block 且為合法 JSON；thinking block 可能在前，需跳過
    let text = parsed["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
        })
        .ok_or("Claude 回應中沒有文字內容")?;

    let result: Value =
        serde_json::from_str(text).map_err(|e| format!("翻譯結果 JSON 解析失敗: {e}"))?;

    let mut map = HashMap::new();
    if let Some(items) = result["translations"].as_array() {
        for entry in items {
            if let (Some(index), Some(text)) = (entry["index"].as_u64(), entry["text"].as_str()) {
                map.insert(index as usize, text.trim().to_string());
            }
        }
    }
    Ok(map)
}

/// 分批呼叫 Claude API 翻譯全部字幕，逐批回報進度
pub async fn translate_all(
    app: &AppHandle,
    api_key: &str,
    mut subtitles: Vec<SubtitleItem>,
) -> Result<Vec<SubtitleItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let total_chunks = subtitles.len().div_ceil(CHUNK_SIZE).max(1);
    let mut context: Vec<String> = Vec::new();

    for chunk_idx in 0..total_chunks {
        let start = chunk_idx * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(subtitles.len());
        let chunk: Vec<SubtitleItem> = subtitles[start..end].to_vec();
        if chunk.is_empty() {
            break;
        }

        let _ = app.emit(
            "pipeline-progress",
            ProgressEvent {
                stage: "translate".into(),
                percent: chunk_idx as f64 / total_chunks as f64 * 100.0,
                message: format!("翻譯中… 第 {}/{} 批", chunk_idx + 1, total_chunks),
            },
        );

        let map = translate_chunk(&client, api_key, &chunk, &context).await?;
        for item in &mut subtitles[start..end] {
            if let Some(zh) = map.get(&item.index) {
                item.zh = zh.clone();
            }
        }

        context = subtitles[start..end]
            .iter()
            .rev()
            .take(CONTEXT_LINES)
            .map(|s| s.ko.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }

    let _ = app.emit(
        "pipeline-progress",
        ProgressEvent {
            stage: "translate".into(),
            percent: 100.0,
            message: "翻譯完成".into(),
        },
    );

    Ok(subtitles)
}
