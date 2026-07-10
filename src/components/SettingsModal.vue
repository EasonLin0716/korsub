<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types";

const emit = defineEmits<{ close: []; saved: [] }>();

const anthropicKey = ref("");
const saving = ref(false);
const error = ref("");

onMounted(async () => {
  try {
    const s = await invoke<Settings>("load_settings");
    anthropicKey.value = s.anthropic_api_key;
  } catch (e) {
    error.value = String(e);
  }
});

async function save() {
  saving.value = true;
  error.value = "";
  try {
    await invoke("save_settings", {
      settings: {
        anthropic_api_key: anthropicKey.value.trim(),
      },
    });
    emit("saved");
    emit("close");
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="modal">
      <h2>API 設定</h2>
      <label>
        Anthropic API Key（Claude 翻譯）
        <input v-model="anthropicKey" type="password" placeholder="sk-ant-..." />
      </label>
      <p class="hint">語音辨識使用本機 whisper.cpp，不需要額外的 API Key。</p>
      <p class="hint">金鑰以明文儲存在本機設定檔，僅供個人使用。</p>
      <p v-if="error" class="error">{{ error }}</p>
      <div class="actions">
        <button @click="emit('close')">取消</button>
        <button class="primary" :disabled="saving" @click="save">儲存</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}
.modal {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 24px;
  width: 460px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
h2 {
  margin: 0;
  font-size: 16px;
}
label {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
}
.hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
}
.error {
  margin: 0;
  color: var(--danger);
  font-size: 13px;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
</style>
