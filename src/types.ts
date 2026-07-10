export interface SubtitleItem {
  index: number;
  /** 開始時間（秒） */
  start: number;
  /** 結束時間（秒） */
  end: number;
  /** 韓文原文 */
  ko: string;
  /** 繁體中文譯文 */
  zh: string;
}

export interface ProgressEvent {
  stage: "model" | "extract" | "transcribe" | "translate" | "burn";
  /** 0-100，-1 表示不確定進度 */
  percent: number;
  message: string;
}

export interface Settings {
  anthropic_api_key: string;
}

export interface EnvStatus {
  ffmpeg: string | null;
  ffprobe: string | null;
  whisper: string | null;
  model_downloaded: boolean;
}
