# KorSub

**English** | [繁體中文](./README.zh-TW.md)

Korean video → Traditional Chinese subtitle generator (Tauri desktop app).

Feed it a Korean-language video and it automatically:

1. **Extracts the audio track** — 16kHz mono WAV via the system FFmpeg
2. **Transcribes speech** — local whisper.cpp (`large-v3-turbo` model, Metal-accelerated on Apple Silicon, fully offline and free)
3. **Translates** — Claude API (`claude-opus-4-8`) translates in batches into natural Taiwanese-style Traditional Chinese, with rolling context
4. **Outputs subtitles** — export `.srt` soft subtitles (default), or burn hard subtitles into a new video with FFmpeg

Subtitles can be previewed and edited line-by-line before export, with an optional bilingual (Chinese + Korean) mode. Previously exported SRT files can be re-imported to skip transcription and translation entirely.

## Architecture

```
Frontend  Vue 3 + TypeScript + Vite (WebView)
            │  Tauri invoke / event (IPC)
Backend   Rust (Tauri 2)
            ├─ ffmpeg.rs    audio extraction, ffprobe, hard-sub burn-in (parses -progress for live progress)
            ├─ whisper.rs   local whisper-cli transcription (JSON output + progress parsing), model download
            ├─ translate.rs Claude API (structured output JSON schema, 25 lines per batch)
            ├─ srt.rs       SRT build/parse (monolingual / bilingual)
            └─ settings.rs  API key stored in the app config directory as settings.json
```

Progress is pushed to the frontend in real time via the `pipeline-progress` event (stages: model / extract / transcribe / translate / burn).

## Requirements

- [FFmpeg](https://ffmpeg.org/): `brew install ffmpeg-full`
  (Note: the core `ffmpeg` formula dropped libass as of v8 and has no `subtitles` filter — **hard-sub burn-in requires `ffmpeg-full`**; the app searches `/opt/homebrew/opt/ffmpeg-full/bin` first)
- [whisper.cpp](https://github.com/ggml-org/whisper.cpp): `brew install whisper-cpp`
- Anthropic API key (for Claude translation) — enter it in the app under "⚙ API 設定"

Both CLIs are resolved automatically from PATH plus `/opt/homebrew/bin` and `/usr/local/bin`.

On first use the app prompts to download the transcription model `ggml-large-v3-turbo.bin` (~1.6GB, stored in the app data directory, one-time only).

The API key is **stored in plaintext** in the local app config directory — avoid using this on shared machines.

## Development

```sh
pnpm install
pnpm tauri dev
```

## Building

```sh
pnpm tauri build
```

## Known limitations (MVP)

- Speech recognition runs locally; only subtitle text is sent to the Anthropic API (audio never leaves the machine)
- End users of a packaged build still need FFmpeg and whisper.cpp installed (no bundled sidecar binaries yet)
- Recognition quality drops on overlapping voices or loud background music — fix lines manually in the table before exporting
