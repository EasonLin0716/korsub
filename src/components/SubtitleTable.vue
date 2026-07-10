<script setup lang="ts">
import type { SubtitleItem } from "../types";

defineProps<{ subtitles: SubtitleItem[]; disabled: boolean }>();

function formatTime(sec: number): string {
  const ms = Math.round(sec * 1000);
  const h = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  const frac = ms % 1000;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(frac).padStart(3, "0")}`;
}
</script>

<template>
  <div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th class="col-idx">#</th>
          <th class="col-time">時間</th>
          <th>韓文原文</th>
          <th>中文譯文（可編輯）</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in subtitles" :key="item.index">
          <td class="col-idx">{{ item.index + 1 }}</td>
          <td class="col-time">
            {{ formatTime(item.start) }}<br />{{ formatTime(item.end) }}
          </td>
          <td class="ko">{{ item.ko }}</td>
          <td>
            <input
              v-model="item.zh"
              type="text"
              :disabled="disabled"
              placeholder="（尚未翻譯）"
            />
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.table-wrap {
  border: 1px solid var(--border);
  border-radius: 10px;
  overflow: auto;
  max-height: 420px;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
th {
  position: sticky;
  top: 0;
  background: var(--panel-2);
  text-align: left;
  padding: 10px 12px;
  color: var(--text-dim);
  font-weight: 600;
  z-index: 1;
}
td {
  padding: 8px 12px;
  border-top: 1px solid var(--border);
  vertical-align: top;
}
.col-idx {
  width: 44px;
  color: var(--text-dim);
}
.col-time {
  width: 110px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  font-size: 12px;
  white-space: nowrap;
}
.ko {
  color: var(--text-dim);
  max-width: 320px;
}
td input {
  padding: 6px 8px;
  font-size: 13px;
}
</style>
