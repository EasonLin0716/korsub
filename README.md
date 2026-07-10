# KorSub

韓文影片 → 繁體中文字幕生成工具（Tauri 桌面應用）。

輸入一部全韓文影片，自動完成：

1. **抽取音軌** — 以系統 FFmpeg 轉出 16kHz 單聲道 WAV
2. **語音辨識** — 本機 whisper.cpp（`large-v3-turbo` 模型，Apple Silicon Metal 加速，完全離線免費）
3. **翻譯** — Claude API（`claude-opus-4-8`）分批翻譯成台灣慣用繁體中文，帶前文脈絡
4. **輸出** — 匯出 `.srt` 軟字幕（預設），或用 FFmpeg 燒錄硬字幕重新輸出影片

字幕在匯出前可於表格中逐條預覽與編輯，支援「中文＋韓文」雙語模式。

## 技術架構

```
前端  Vue 3 + TypeScript + Vite（WebView）
        │  Tauri invoke / event (IPC)
後端  Rust (Tauri 2)
        ├─ ffmpeg.rs    抽音軌、ffprobe、硬字幕燒錄（解析 -progress 回報進度）
        ├─ whisper.rs   本機 whisper-cli 辨識（JSON 輸出 + 進度解析）、模型下載
        ├─ translate.rs Claude API（structured output JSON schema，每批 25 條）
        ├─ srt.rs       SRT 組裝（單語 / 雙語）
        └─ settings.rs  API 金鑰存於 app config 目錄 settings.json
```

進度透過 `pipeline-progress` 事件即時推送到前端（stage: model / extract / transcribe / translate / burn）。

## 需求

- [FFmpeg](https://ffmpeg.org/)：`brew install ffmpeg-full`
  （注意：核心的 `ffmpeg` formula 自 v8 起不含 libass，沒有 `subtitles` filter，**燒錄硬字幕必須用 `ffmpeg-full`**；App 會優先搜尋 `/opt/homebrew/opt/ffmpeg-full/bin`）
- [whisper.cpp](https://github.com/ggml-org/whisper.cpp)：`brew install whisper-cpp`
- Anthropic API Key（Claude 翻譯）— App 內「⚙ API 設定」填入

以上兩個 CLI 會自動搜尋 PATH 與 `/opt/homebrew/bin`、`/usr/local/bin`。

首次使用會提示下載辨識模型 `ggml-large-v3-turbo.bin`（約 1.6GB，存於 app 資料目錄，只需一次）。

API 金鑰**以明文儲存在本機** app config 目錄，請勿在共用機器上使用。

## 開發

```sh
pnpm install
pnpm tauri dev
```

## 打包

```sh
pnpm tauri build
```

## 已知限制（MVP）

- 語音辨識在本機執行，僅翻譯文字會送到 Anthropic API（音訊不出本機）
- 打包發佈時使用者機器仍需自行安裝 FFmpeg 與 whisper.cpp（未內嵌 sidecar binary）
- 韓文人聲重疊、背景音樂大的片段辨識品質會下降，可在表格中手動修正後再匯出
