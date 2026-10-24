<script setup lang="ts">
/** Display actual storage location and explicitly unavailable preferences. */
import { ref, onMounted } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { showHeroBanner, saveInterval, saveIntervals, configureSaving, flushReadingProgress } from "../stores/library";

const exporting = ref(false);
const exportStatus = ref("");

/** Flush the current reading position before requesting an explicit native JSON export. */
async function onExport(): Promise<void> {
  if (!isTauri() || exporting.value) return;
  exporting.value = true;
  exportStatus.value = "";
  try {
    await flushReadingProgress();
    const destination = await invoke<string | null>("export_library_state");
    exportStatus.value = destination ? "已导出：" + destination : "已取消导出";
  } catch (error) {
    exportStatus.value = "导出失败：" + error;
  } finally { exporting.value = false; }
}

/** Map the five slider positions to the supported save intervals in seconds. */
function onInterval(event: Event): void {
  const control = event.target;
  if (!(control instanceof HTMLInputElement)) return;
  const seconds = saveIntervals[Number(control.value)];
  if (seconds !== undefined) configureSaving(seconds);
}

// Resolve storage from the host instead of embedding a developer's filesystem.
const storagePath = ref(isTauri() ? "正在读取…" : "浏览器内存（刷新后清空）");
onMounted(async () => {
  if (!isTauri()) return;
  try {
    storagePath.value = await invoke<string>("storage_location");
  } catch (error) {
    storagePath.value = "读取目录失败：" + error;
  }
});
</script>

<template>
  <div class="settings-container">
    <header class="settings-header">
      <h1 class="page-title">系统设置</h1>
      <span class="mode-badge dev">{{ isTauri() ? "桌面原型" : "浏览器预览" }}</span>
    </header>
    <section class="config-section">
      <h2 class="section-title">阅读进度</h2>
      <div class="config-card">
        <div class="config-row">
          <div class="config-meta">
            <label class="config-name" for="save-interval">自动保存间隔：{{ saveInterval }} 秒</label>
            <span class="config-desc">持续阅读时按所选间隔保存最新位置；切换后台、返回书架和正常关闭时提前保存。</span>
            <input id="save-interval" class="interval-slider" type="range" min="0" max="4" step="1"
              :value="saveIntervals.findIndex(seconds => seconds === saveInterval)" :aria-valuetext="saveInterval + ' 秒'" @input="onInterval" />
            <div class="interval-labels" aria-hidden="true"><span v-for="seconds in saveIntervals" :key="seconds">{{ seconds }} 秒</span></div>
          </div>
        </div>
      </div>
    </section>
    <section class="config-section">
      <h2 class="section-title">💾 数据与存储位置</h2>
      <div class="config-card">
        <div class="config-row">
          <div class="config-meta">
            <span class="config-name">当前数据目录</span>
            <span class="config-desc">开发构建使用当前仓库的 data 目录；发布构建使用系统应用数据目录。</span>
            <div class="path-display-box"><span class="path-text">{{ storagePath }}</span></div>
            <div class="sub-paths">
              <span>storage/saga.db：书架信息、阅读进度与核验决定</span>
              <span>books/&lt;id&gt;.saga：单文件归档存储正文与分块数据</span>
            </div>
          </div>
          <div class="actions-col">
            <button class="btn-secondary" disabled title="尚未实现目录迁移">更改目录（未实现）</button>
          </div>
        </div>
        <div class="config-row">
          <div class="config-meta">
            <span class="config-name">导出书架与阅读记录</span>
            <span class="config-desc">以可阅读的 JSON 保存书架信息、阅读位置与核验决定，不包含书籍正文。</span>
            <span v-if="exportStatus" class="config-desc" role="status">{{ exportStatus }}</span>
          </div>
          <button class="btn-secondary" :disabled="!isTauri() || exporting" @click="onExport">{{ exporting ? '正在导出…' : '导出 JSON' }}</button>
        </div>
      </div>
    </section>
    <section class="config-section">
      <h2 class="section-title">🎙️ 伴读与语音</h2>
      <div class="config-card">
        <div class="config-row">
          <div class="config-meta">
            <span class="config-name">语音合成尚未接入</span>
            <span class="config-desc">阅读器中的播放操作仅演示句子高亮，不会生成声音。语音、时间对齐和跟随滚动待实现。</span>
          </div>
        </div>
      </div>
    </section>
    <section class="config-section">
      <h2 class="section-title">🎨 外观与书架</h2>
      <div class="config-card">
        <div class="config-row">
          <div class="config-meta">
            <span class="config-name">横幅显示当前阅读</span>
            <span class="config-desc">在书架顶部以巨幅焦点卡片展示正在阅读的书籍。关闭后书架将以全平铺封面流呈现。</span>
          </div>
          <div class="toggle-col">
            <label class="switch">
              <input type="checkbox" v-model="showHeroBanner" />
              <span class="slider round"></span>
            </label>
          </div>
        </div>
        <div class="config-row">
          <div class="config-meta">
            <span class="config-name">当前使用深色主题</span>
            <span class="config-desc">可在阅读器调整本次会话的字号；主题切换及偏好持久化尚未实现。</span>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.interval-slider {
  width: 100%;
  max-width: 480px;
  margin: 20px 0 8px;
  accent-color: #38bdf8;
}

.interval-labels {
  display: flex;
  justify-content: space-between;
  max-width: 480px;
  color: #94a3b8;
  font-size: 12px;
}

.settings-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 32px 40px;
  overflow-y: auto;
  user-select: none;
}

.settings-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.page-title {
  font-size: 26px;
  font-weight: 700;
  color: #f8fafc;
}

.mode-badge {
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 12px;
  font-weight: 600;
}

.mode-badge.dev {
  background: rgba(245, 158, 11, 0.15);
  border: 1px solid rgba(245, 158, 11, 0.35);
  color: #fbbf24;
}

.config-section {
  margin-bottom: 30px;
}

.section-title {
  font-size: 15px;
  font-weight: 600;
  color: #cbd5e1;
  margin-bottom: 12px;
}

.config-card {
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 12px;
  overflow: hidden;
}

.config-row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 18px 22px;
  border-bottom: 1px solid #334155;
  gap: 20px;
}

.config-row:last-child {
  border-bottom: none;
}

.config-meta {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.config-name {
  font-size: 14px;
  font-weight: 600;
  color: #f1f5f9;
}

.config-desc {
  font-size: 12px;
  color: #94a3b8;
  margin-top: 4px;
  line-height: 1.5;
}

.path-display-box {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #0f172a;
  border: 1px solid #334155;
  border-radius: 6px;
  padding: 8px 12px;
  margin-top: 10px;
  max-width: 580px;
}

.path-icon {
  font-size: 14px;
}

.path-text {
  font-size: 12px;
  color: #38bdf8;
  font-family: monospace;
  font-weight: 600;
  flex: 1;
}

.path-tag {
  font-size: 10px;
  background: rgba(56, 189, 248, 0.15);
  color: #38bdf8;
  border: 1px solid rgba(56, 189, 248, 0.3);
  padding: 1px 6px;
  border-radius: 4px;
}

.sub-paths {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-family: monospace;
  font-size: 11px;
  color: #64748b;
  margin-top: 8px;
  padding-left: 6px;
}

.sub-paths strong {
  color: #94a3b8;
}

.actions-col {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.btn-primary {
  background: #3b82f6;
  border: none;
  color: white;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}

.btn-primary:hover {
  background: #2563eb;
}

.btn-secondary {
  background: #0f172a;
  border: 1px solid #334155;
  color: #cbd5e1;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}

.btn-secondary:hover {
  background: #334155;
  color: #38bdf8;
}

.config-select {
  background: #0f172a;
  border: 1px solid #334155;
  color: #f1f5f9;
  border-radius: 6px;
  padding: 8px 14px;
  font-size: 13px;
  outline: none;
}

.config-checkbox {
  width: 18px;
  height: 18px;
  accent-color: #3b82f6;
  cursor: pointer;
}

.toggle-col {
  display: flex;
  align-items: center;
}

.switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}

.switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: #334155;
  transition: 0.2s;
  border-radius: 24px;
}

.slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: #f8fafc;
  transition: 0.2s;
  border-radius: 50%;
}

input:checked + .slider {
  background-color: #38bdf8;
}

input:checked + .slider:before {
  transform: translateX(20px);
}
</style>
