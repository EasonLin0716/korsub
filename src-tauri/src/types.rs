use serde::{Deserialize, Serialize};

/// 單條字幕：Whisper 產出的時間戳與韓文原文，翻譯後補上中文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleItem {
    pub index: usize,
    /// 開始時間（秒）
    pub start: f64,
    /// 結束時間（秒）
    pub end: f64,
    /// 韓文原文
    pub ko: String,
    /// 繁體中文譯文（翻譯前為空字串）
    #[serde(default)]
    pub zh: String,
}

/// 進度事件，透過 `pipeline-progress` 事件送到前端
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    /// extract | transcribe | translate | burn
    pub stage: String,
    /// 0.0 ~ 100.0，-1 表示不確定（spinner）
    pub percent: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub anthropic_api_key: String,
}
