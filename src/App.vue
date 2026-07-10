<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import SettingsModal from "./components/SettingsModal.vue";
import SubtitleTable from "./components/SubtitleTable.vue";
import type { EnvStatus, ProgressEvent, Settings, SubtitleItem } from "./types";

const videoPath = ref<string | null>(null);
const subtitles = ref<SubtitleItem[]>([]);
const busy = ref(false);
const progress = ref<ProgressEvent | null>(null);
const errorMsg = ref("");
const statusMsg = ref("");
const showSettings = ref(false);
const bilingual = ref(false);

const env = ref<EnvStatus>({
  ffmpeg: null,
  ffprobe: null,
  whisper: null,
  model_downloaded: false,
});
const hasAnthropicKey = ref(false);

const videoName = computed(() =>
  videoPath.value ? videoPath.value.split("/").pop() : null,
);
const hasTranscript = computed(() => subtitles.value.length > 0);
const hasTranslation = computed(() =>
  subtitles.value.some((s) => s.zh.trim() !== ""),
);

const stageLabels: Record<string, string> = {
  model: "下載辨識模型",
  extract: "抽取音軌",
  transcribe: "語音辨識",
  translate: "翻譯",
  burn: "燒錄硬字幕",
};

const whisperReady = computed(
  () => env.value.whisper !== null && env.value.model_downloaded,
);

let unlisten: UnlistenFn | null = null;

onMounted(async () => {
  unlisten = await listen<ProgressEvent>("pipeline-progress", (e) => {
    progress.value = e.payload;
  });
  env.value = await invoke<EnvStatus>("check_environment");
  await refreshKeyStatus();
});

onUnmounted(() => unlisten?.());

async function refreshKeyStatus() {
  const s = await invoke<Settings>("load_settings");
  hasAnthropicKey.value = s.anthropic_api_key.trim() !== "";
}

async function downloadModel() {
  busy.value = true;
  errorMsg.value = "";
  statusMsg.value = "";
  try {
    await invoke("download_model");
    env.value = await invoke<EnvStatus>("check_environment");
    statusMsg.value = "辨識模型下載完成";
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    busy.value = false;
    progress.value = null;
  }
}

async function pickVideo() {
  const selected = await open({
    multiple: false,
    filters: [
      { name: "影片", extensions: ["mp4", "mkv", "mov", "avi", "webm", "ts", "m4v"] },
    ],
  });
  if (typeof selected === "string") {
    videoPath.value = selected;
    // 不清空字幕：允許「匯入 SRT → 選影片 → 直接燒錄」；重新辨識會覆蓋
    errorMsg.value = "";
    statusMsg.value = "";
    progress.value = null;
  }
}

async function runTranscribe() {
  if (!videoPath.value) return;
  busy.value = true;
  errorMsg.value = "";
  statusMsg.value = "";
  try {
    subtitles.value = await invoke<SubtitleItem[]>("transcribe_video", {
      videoPath: videoPath.value,
    });
    statusMsg.value = `辨識完成，共 ${subtitles.value.length} 條字幕`;
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    busy.value = false;
    progress.value = null;
  }
}

async function runTranslate() {
  if (!hasTranscript.value) return;
  busy.value = true;
  errorMsg.value = "";
  statusMsg.value = "";
  try {
    subtitles.value = await invoke<SubtitleItem[]>("translate_subtitles", {
      subtitles: subtitles.value,
    });
    statusMsg.value = "翻譯完成，可直接在表格中修改譯文";
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    busy.value = false;
    progress.value = null;
  }
}

async function importSrt() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "SRT 字幕", extensions: ["srt"] }],
  });
  if (typeof selected !== "string") return;
  errorMsg.value = "";
  statusMsg.value = "";
  try {
    subtitles.value = await invoke<SubtitleItem[]>("import_srt", {
      path: selected,
    });
    statusMsg.value = `已匯入 ${subtitles.value.length} 條字幕，可直接編輯、匯出或燒錄`;
  } catch (e) {
    errorMsg.value = String(e);
  }
}

function baseName(): string {
  const name = videoName.value ?? "subtitle";
  return name.replace(/\.[^.]+$/, "");
}

async function exportSrt() {
  const path = await save({
    defaultPath: `${baseName()}.zh.srt`,
    filters: [{ name: "SRT 字幕", extensions: ["srt"] }],
  });
  if (!path) return;
  errorMsg.value = "";
  try {
    await invoke("export_srt", {
      subtitles: subtitles.value,
      path,
      bilingual: bilingual.value,
    });
    statusMsg.value = `已匯出 SRT：${path}`;
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function burnSubtitles() {
  if (!videoPath.value) return;
  const path = await save({
    defaultPath: `${baseName()}.zh.mp4`,
    filters: [{ name: "MP4 影片", extensions: ["mp4"] }],
  });
  if (!path) return;
  busy.value = true;
  errorMsg.value = "";
  statusMsg.value = "";
  try {
    await invoke("burn_subtitles", {
      videoPath: videoPath.value,
      subtitles: subtitles.value,
      outputPath: path,
      bilingual: bilingual.value,
    });
    statusMsg.value = `硬字幕影片已輸出：${path}`;
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    busy.value = false;
    progress.value = null;
  }
}
</script>

<template>
  <header>
    <div>
      <h1>KorSub</h1>
      <p class="subtitle">韓文影片 → 繁體中文字幕</p>
    </div>
    <button @click="showSettings = true">⚙ API 設定</button>
  </header>

  <div v-if="!env.ffmpeg" class="banner danger">
    找不到 FFmpeg，請先安裝：<code>brew install ffmpeg</code>
  </div>
  <div v-if="env.ffmpeg && !env.whisper" class="banner danger">
    找不到 whisper.cpp，請先安裝：<code>brew install whisper-cpp</code>
  </div>
  <div
    v-if="env.whisper && !env.model_downloaded"
    class="banner warn model-banner"
  >
    <span>首次使用需下載語音辨識模型（約 1.6GB，只需一次）</span>
    <button class="primary" :disabled="busy" @click="downloadModel">
      下載辨識模型
    </button>
  </div>
  <div v-if="!hasAnthropicKey" class="banner warn">
    尚未設定 Anthropic API Key（Claude 翻譯用），請點右上角「API 設定」。
  </div>

  <section class="panel">
    <div class="step-row">
      <span class="step-num">1</span>
      <div class="step-body">
        <div class="step-title">選擇影片</div>
        <div class="step-actions">
          <button class="primary" :disabled="busy" @click="pickVideo">
            選擇影片檔案
          </button>
          <button :disabled="busy" @click="importSrt" title="匯入先前匯出的 SRT，跳過辨識與翻譯">
            匯入 SRT 字幕
          </button>
          <span v-if="videoName" class="file-name">{{ videoName }}</span>
        </div>
      </div>
    </div>

    <div class="step-row">
      <span class="step-num">2</span>
      <div class="step-body">
        <div class="step-title">語音辨識（本機 whisper.cpp）</div>
        <div class="step-actions">
          <button
            class="primary"
            :disabled="busy || !videoPath || !whisperReady"
            @click="runTranscribe"
          >
            開始辨識
          </button>
        </div>
      </div>
    </div>

    <div class="step-row">
      <span class="step-num">3</span>
      <div class="step-body">
        <div class="step-title">翻譯成繁體中文（Claude）</div>
        <div class="step-actions">
          <button
            class="primary"
            :disabled="busy || !hasTranscript || !hasAnthropicKey"
            @click="runTranslate"
          >
            開始翻譯
          </button>
        </div>
      </div>
    </div>
  </section>

  <section v-if="busy && progress" class="panel progress-panel">
    <div class="progress-label">
      <strong>{{ stageLabels[progress.stage] ?? progress.stage }}</strong>
      <span>{{ progress.message }}</span>
    </div>
    <div class="progress-track">
      <div
        class="progress-fill"
        :class="{ indeterminate: progress.percent < 0 }"
        :style="progress.percent >= 0 ? { width: progress.percent + '%' } : {}"
      ></div>
    </div>
  </section>

  <p v-if="errorMsg" class="banner danger">{{ errorMsg }}</p>
  <p v-if="statusMsg" class="banner success">{{ statusMsg }}</p>

  <section v-if="hasTranscript" class="panel">
    <div class="table-header">
      <h2>字幕預覽（{{ subtitles.length }} 條）</h2>
      <div class="export-actions">
        <label class="checkbox">
          <input v-model="bilingual" type="checkbox" />
          雙語字幕（中文＋韓文）
        </label>
        <button :disabled="busy || !hasTranslation" @click="exportSrt">
          匯出 SRT
        </button>
        <button :disabled="busy || !hasTranslation" @click="burnSubtitles">
          燒錄硬字幕
        </button>
      </div>
    </div>
    <SubtitleTable :subtitles="subtitles" :disabled="busy" />
  </section>

  <SettingsModal
    v-if="showSettings"
    @close="showSettings = false"
    @saved="refreshKeyStatus"
  />
</template>

<style scoped>
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}
.subtitle {
  margin: 4px 0 0;
  color: var(--text-dim);
  font-size: 13px;
}
.panel {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 18px 20px;
  margin-bottom: 16px;
}
.step-row {
  display: flex;
  gap: 14px;
  align-items: flex-start;
  padding: 10px 0;
}
.step-row + .step-row {
  border-top: 1px solid var(--border);
}
.step-num {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: var(--panel-2);
  border: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--text-dim);
  flex-shrink: 0;
  margin-top: 2px;
}
.step-body {
  flex: 1;
}
.step-title {
  font-weight: 600;
  margin-bottom: 8px;
}
.step-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.file-name {
  color: var(--text-dim);
  font-size: 13px;
  word-break: break-all;
}
.banner {
  border-radius: 10px;
  padding: 12px 16px;
  margin-bottom: 16px;
  font-size: 13px;
  border: 1px solid;
}
.banner.danger {
  background: rgba(248, 113, 113, 0.08);
  border-color: rgba(248, 113, 113, 0.4);
  color: var(--danger);
}
.banner.warn {
  background: rgba(250, 204, 21, 0.07);
  border-color: rgba(250, 204, 21, 0.35);
  color: #facc15;
}
.model-banner {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}
.banner.success {
  background: rgba(74, 222, 128, 0.07);
  border-color: rgba(74, 222, 128, 0.35);
  color: var(--success);
}
.progress-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.progress-label {
  display: flex;
  gap: 12px;
  align-items: baseline;
}
.progress-label span {
  color: var(--text-dim);
  font-size: 13px;
}
.progress-track {
  height: 8px;
  background: var(--panel-2);
  border-radius: 4px;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 4px;
  transition: width 0.3s ease;
}
.progress-fill.indeterminate {
  width: 35%;
  animation: slide 1.2s ease-in-out infinite;
}
@keyframes slide {
  0% {
    margin-left: -35%;
  }
  100% {
    margin-left: 100%;
  }
}
.table-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.table-header h2 {
  margin: 0;
  font-size: 15px;
}
.export-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.checkbox {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
  cursor: pointer;
}
code {
  background: var(--panel-2);
  padding: 2px 6px;
  border-radius: 4px;
}
</style>
