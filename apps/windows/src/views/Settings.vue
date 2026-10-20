<script setup lang="ts">
/** Display actual storage location and explicitly unavailable preferences. */
import { ref, onMounted } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";

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
      <h2 class="section-title">💾 数据与存储位置</h2>
      <div class="config-card">
        <div class="config-row">
          <div class="config-meta">
            <span class="config-name">当前数据目录</span>
            <span class="config-desc">开发构建使用当前仓库的 data 目录；发布构建使用系统应用数据目录。</span>
            <div class="path-display-box"><span class="path-text">{{ storagePath }}</span></div>
            <div class="sub-paths">
              <span>storage/saga.db：书籍、章节与核验决定</span>
              <span>cache/：导入原始文件缓存</span>
            </div>
          </div>
          <div class="actions-col">
            <button class="btn-secondary" disabled title="尚未实现目录迁移">更改目录（未实现）</button>
          </div>
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
      <h2 class="section-title">🎨 外观</h2>
      <div class="config-card">
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
</style>
