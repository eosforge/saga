<script setup lang="ts">
/** TXT import, catalog filtering, and native event subscription lifecycle. */
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  books,
  failure,
  activeBook,
  selectBook,
  isPlaying,
  togglePlay,
  hasAudio,
  importBook,
  importNovel,
  deleteBook,
  loadStoredNovels,
  activeAuditReport,
  activeAuditNovel,
  showHeroBanner,
  isGlobalBusy,
  type BookItem
} from "../stores/library";
import type { NovelImportResult } from "../../../../protocol/bindings/NovelImportResult";
import type { ImportProgress } from "../../../../protocol/bindings/ImportProgress";

const emit = defineEmits<{
  (e: "open-reader"): void;
  (e: "open-audit"): void;
}>();

const fileInputRef = ref<HTMLInputElement | null>(null);
const toastMessage = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;
let unlistenDragDrop: (() => void) | null = null;
let unlistenProgress: (() => void) | null = null;
let progressReady: Promise<void> | undefined;
let disposed = false;

const isPicking = ref(false);
const isImporting = ref(false);
const importPhaseTitle = ref("正在导入");
const importFileName = ref("");
const importDetailText = ref("");
const importPercent = ref(0);
const importMeasured = ref(false);
const bookToDelete = ref<BookItem | null>(null);

watch(isImporting, (val) => {
  if (isGlobalBusy) isGlobalBusy.value = val;
});

function showToast(msg: string): void {
  toastMessage.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = null;
  }, 4000);
}

function getBookPriority(b: BookItem): number {
  if (b.progress > 0 && b.progress < 100) return 0; // 正在读 (最高优)
  if (b.progress === 0) return 1;                  // 未读 / 新上架
  return 2;                                        // 已读完 (沉底)
}

const sortedBooks = computed(() => {
  return books.value.slice().sort((a, b) => {
    const pa = getBookPriority(a);
    const pb = getBookPriority(b);
    if (pa !== pb) return pa - pb;
    return a.title.localeCompare(b.title, "zh-CN");
  });
});

function handleOpenBook(id: string): void {
  selectBook(id);
  emit("open-reader");
}

function handleHeroPlay(): void {
  togglePlay();
}

function handleImportResult(result: NovelImportResult, format: "TXT" | "SAGA" = "TXT"): void {
  const { novel, validation } = result;
  importNovel(novel, format);
  isImporting.value = false;

  const pending = validation?.anomalies?.filter(item => !item.retained).length || 0;
  if (pending > 0) {
    activeAuditReport.value = validation;
    activeAuditNovel.value = novel;
    showToast(`《${novel.book.title}》有 ${pending} 处待核验，正在进入逐项核验...`);
    emit("open-audit");
  } else {
    showToast(`《${novel.book.title}》导入成功，共 ${novel.sections.length} 章节，无待处理项`);
    emit("open-reader");
  }
}

async function subscribeProgress(): Promise<void> {
  progressReady ??= (async () => {
    const { listen } = await import("@tauri-apps/api/event");
    const unlisten = await listen<ImportProgress>("import-progress", (event) => {
      if (disposed || !isImporting.value) return;
      importPhaseTitle.value = event.payload.title;
      importDetailText.value = event.payload.detail;
      importPercent.value = Math.max(importPercent.value, Math.min(99, event.payload.percent));
      importMeasured.value = event.payload.phase === "reading" || event.payload.phase === "parsing";
    });
    if (disposed) unlisten();
    else unlistenProgress = unlisten;
  })();
  await progressReady;
}

async function triggerFileImport(): Promise<void> {
  if (isPicking.value || isImporting.value) return;
  if (!isTauri()) {
    fileInputRef.value?.click();
    return;
  }

  isPicking.value = true;
  let path: string | null = null;
  try {
    path = await invoke<string | null>("pick_novel_file");
  } catch (err) {
    showToast(`打开文件选择框失败: ${err}`);
  } finally {
    isPicking.value = false;
  }
  if (path && !disposed) await importFilePath(path);
}

function promptDeleteBook(book: BookItem, event?: Event): void {
  event?.stopPropagation();
  bookToDelete.value = book;
}

function cancelDelete(): void {
  bookToDelete.value = null;
}

async function confirmDelete(): Promise<void> {
  if (!bookToDelete.value) return;
  const target = bookToDelete.value;
  try {
    await deleteBook(target.id);
    showToast(`《${target.title}》已删除`);
  } catch (err) {
    console.error("Delete book failed:", err);
    showToast(`删除失败: ${err}`);
  } finally {
    bookToDelete.value = null;
  }
}

async function importFilePath(path: string): Promise<void> {
  if (isPicking.value || isImporting.value || disposed) return;
  const fileName = path.split(/[\\/]/).pop() || path;

  isImporting.value = true;
  importPhaseTitle.value = "正在导入";
  importFileName.value = fileName;
  importPercent.value = 0;
  importMeasured.value = false;
  importDetailText.value = "正在准备读取文件...";

  try {
    // Register before starting work so the initial read updates cannot be lost.
    await subscribeProgress();
    await nextTick();
    if (disposed) return;
    const result = await invoke<NovelImportResult>("import_txt_file", { path });
    if (disposed) return;
    importPercent.value = 100;
    const ext = fileName.split(".").pop()?.toUpperCase() || "TXT";
    handleImportResult(result, ext === "SAGA" ? "SAGA" : "TXT");
  } catch (err) {
    console.error("Native import_txt_file failed:", err);
    showToast(`导入失败: ${err}`);
  } finally {
    isImporting.value = false;
  }
}

async function processImportedFile(file: File): Promise<void> {
  if (isPicking.value || isImporting.value || disposed) return;
  const fileName = file.name;
  const dotIndex = fileName.lastIndexOf(".");
  const title = dotIndex > 0 ? fileName.substring(0, dotIndex) : fileName;
  const ext = dotIndex > 0 ? fileName.substring(dotIndex + 1).toUpperCase() : "TXT";
  if (ext !== "TXT" && ext !== "SAGA") { showToast("目前仅支持 TXT 与 .saga 文件"); return; }
  const format = ext === "SAGA" ? "SAGA" : "TXT";

  try {
    isImporting.value = true;
    importPhaseTitle.value = "正在导入";
    importFileName.value = fileName;
    importPercent.value = 0;
    importMeasured.value = false;
    importDetailText.value = "正在读取文件文本...";

    const text = await file.text();
    if (disposed) return;
    importPercent.value = 60;
    importPhaseTitle.value = "正在解析";
    importDetailText.value = "正在切片章节与段落...";

    importBook(title, "本地导入", format, text);
    importPercent.value = 100;
    showToast(`《${title}》已导入浏览器内存`);
    isImporting.value = false;
    emit("open-reader");
  } catch (err) {
    console.error("Browser import failed:", err);
    showToast(`导入失败: ${err}`);
    isImporting.value = false;
  }
}

async function handleFileSelected(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;

  await processImportedFile(file);
  target.value = "";
}

async function handleDrop(event: DragEvent): Promise<void> {
  event.preventDefault();
  const file = event.dataTransfer?.files?.[0];
  if (file) {
    await processImportedFile(file);
  }
}

onMounted(async () => {
  if (isTauri()) {
    try {
      await subscribeProgress();
    } catch (e) {
      console.warn("Native progress listener not available:", e);
    }
  }
  try { await loadStoredNovels(); } catch { /* The shared error banner reports the failure. */ }
  if (disposed || !isTauri()) return;

  try {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    const webview = getCurrentWebview();
    const unlisten = await webview.onDragDropEvent(async (event) => {
      if (event.payload.type === "drop") {
        const paths = event.payload.paths;
        if (paths && paths.length > 0) {
          const targetPath = paths[0];
          const lower = targetPath.toLowerCase();
          if (lower.endsWith(".txt") || lower.endsWith(".saga")) {
            await importFilePath(targetPath);
          }
        }
      }
    });
    if (disposed) unlisten();
    else unlistenDragDrop = unlisten;
  } catch (e) {
    console.warn("Native drag drop listener not available in pure web mode:", e);
  }
});

onUnmounted(() => {
  disposed = true;
  if (isGlobalBusy) isGlobalBusy.value = false;
  if (toastTimer) clearTimeout(toastTimer);
  if (unlistenDragDrop) {
    unlistenDragDrop();
  }
  if (unlistenProgress) {
    unlistenProgress();
  }
});
</script>

<template>
  <div class="shelf-container" @dragover.prevent @drop="handleDrop">
    <p v-if="failure" role="alert">{{ failure }}</p>
    <p v-if="!isTauri()">浏览器临时预览：仅按原文展示 TXT，刷新后清空；章节解析和持久化需使用桌面端。</p>
    <!-- Toast Feedback Notification -->
    <transition name="toast-fade">
      <div v-if="toastMessage" class="shelf-toast">
        <span class="toast-icon">✨</span>
        <span class="toast-text">{{ toastMessage }}</span>
      </div>
    </transition>
    <!-- Hidden File Input for Native / Browser Picking -->
    <input
      ref="fileInputRef"
      type="file"
      accept=".txt,.saga"
      class="hidden-file-input"
      @change="handleFileSelected"
    />

    <!-- Hero Card: Continue Reading Focus (shown when activeBook exists and enabled in settings) -->
    <section v-if="showHeroBanner && activeBook" class="hero-section">
      <div class="hero-card">
        <div class="hero-cover" :style="{ background: activeBook.coverGradient }">
          <div class="hero-cover-inner">
            <span class="hero-cover-char">{{ activeBook.title[0] || '书' }}</span>
          </div>
        </div>

        <div class="hero-details">
          <div class="hero-label-row">
            <span class="hero-badge">⚡ 最近在读</span>
            <span class="hero-timestamp">{{ activeBook.lastRead }}</span>
          </div>
          <h2 class="hero-title">{{ activeBook.title }}</h2>
          <p class="hero-author">{{ activeBook.author }} · {{ activeBook.currentChapter }}</p>
          <p v-if="activeBook.description" class="hero-desc" :title="activeBook.description">{{ activeBook.description }}</p>

          <div class="hero-progress-block">
            <div class="progress-bar-wrap">
              <div class="progress-bar-fill" :style="{ width: `${activeBook.progress}%` }"></div>
            </div>
            <span class="progress-text">已阅读 {{ activeBook.progress }}%</span>
          </div>

          <div class="hero-actions">
            <button class="primary-read-btn" @click="handleOpenBook(activeBook.id)">
              <svg class="btn-svg-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
                <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
              </svg>
              <span>继续阅读</span>
            </button>
            <button v-if="hasAudio" class="secondary-audio-btn" @click="handleHeroPlay">
              <svg v-if="isPlaying" class="btn-svg-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="6" y="4" width="4" height="16" rx="1" />
                <rect x="14" y="4" width="4" height="16" rx="1" />
              </svg>
              <svg v-else class="btn-svg-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
              <span>{{ isPlaying ? "暂停伴读" : "继续伴读" }}</span>
            </button>
            <button
              class="hero-delete-btn"
              @click="promptDeleteBook(activeBook)"
              title="删除当前书籍 (清除本地缓存与数据库记录)"
            >
              <svg class="btn-svg-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M3 6h18m-2 0v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6m3 0V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                <line x1="10" y1="11" x2="10" y2="17" />
                <line x1="14" y1="11" x2="14" y2="17" />
              </svg>
              <span>删除书籍</span>
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- Divider between Hero Banner and Books Grid -->
    <div v-if="showHeroBanner && activeBook" class="shelf-divider"></div>

    <!-- Books Grid -->
    <div class="books-grid">
      <div
        v-for="b in sortedBooks"
        :key="b.id"
        class="book-card"
        @click="handleOpenBook(b.id)"
      >
        <div class="card-cover" :style="{ background: b.coverGradient }">
          <button
            class="card-delete-btn"
            @click.stop="promptDeleteBook(b, $event)"
            title="删除书籍 (清除本地缓存与数据库记录)"
            aria-label="删除书籍"
          >
            <svg class="card-delete-svg" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3 6h18m-2 0v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6m3 0V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
              <line x1="10" y1="11" x2="10" y2="17" />
              <line x1="14" y1="11" x2="14" y2="17" />
            </svg>
          </button>
          <span class="card-cover-title">{{ b.title }}</span>
          <p v-if="b.description" class="card-cover-desc" :title="b.description">{{ b.description }}</p>
          <div class="card-glow"></div>
        </div>

        <div class="card-meta">
          <h3 class="card-title" :title="b.title">{{ b.title }}</h3>
          <p class="card-author">{{ b.author }}</p>
          <div class="card-progress-row">
            <div class="card-progress-track">
              <div class="card-progress-thumb" :style="{ width: `${b.progress}%` }"></div>
            </div>
            <span class="card-progress-num">
              {{ b.progress >= 100 ? '已读完' : b.progress > 0 ? `${b.progress}%` : '未读' }}
            </span>
          </div>
        </div>
      </div>

      <!-- Add Book Drop Card -->
      <div class="add-card" @click="triggerFileImport">
        <div class="add-icon">＋</div>
        <span class="add-label">导入本地文件</span>
      </div>
    </div>

    <!-- Import Loading Spinner Overlay -->
    <transition name="overlay-fade">
      <div v-if="isImporting" class="import-loading-overlay">
        <div class="import-spinner-card">
          <div class="spinner-ring"></div>
          <h3 class="spinner-title">{{ importPhaseTitle }}</h3>
          <p class="spinner-filename" v-if="importFileName">{{ importFileName }}</p>
          
          <div class="import-progress-container">
            <div
              class="import-progress-track"
              role="progressbar"
              :aria-label="importPhaseTitle"
              :aria-valuenow="importPercent"
              :aria-valuemin="0"
              :aria-valuemax="100"
            >
              <div class="import-progress-fill" :style="{ width: `${importPercent}%` }"></div>
            </div>
            <div class="import-progress-meta">
              <span class="import-progress-detail">{{ importDetailText || '处理中...' }}</span>
              <span class="import-progress-percent">{{ importPercent }}%</span>
            </div>
          </div>

          <div class="spinner-lock-hint">
            <span>🔒 已暂停其余操作，请稍候...</span>
          </div>
        </div>
      </div>
    </transition>

    <!-- Delete Confirmation Modal Dialog -->
    <transition name="modal-fade">
      <div v-if="bookToDelete" class="modal-backdrop" @click.self="cancelDelete">
        <div class="delete-dialog-card">
          <div class="delete-dialog-header">
            <div class="delete-warn-icon">⚠️</div>
            <div class="delete-header-info">
              <h3 class="delete-dialog-title">确认删除书籍</h3>
              <span class="delete-dialog-sub">此操作将同步清除本地物理缓存与数据库记录</span>
            </div>
          </div>

          <div class="delete-dialog-body">
            确定要从书架彻底删除 <strong>《{{ bookToDelete.title }}》</strong> 吗？
            <div class="delete-warning-box">
              <div class="warning-box-title">❗ 本地数据清除清单：</div>
              <ul class="warning-box-list">
                <li>书籍单文件归档：<code>data/books/{{ bookToDelete.id }}.saga</code></li>
                <li>SQLite 数据库：<code>saga.db</code> 中关联的全部卷、章节与切片索引</li>
                <li>此操作不可撤销，请谨慎操作。</li>
              </ul>
            </div>
          </div>

          <div class="delete-dialog-footer">
            <button class="modal-btn-cancel" @click="cancelDelete">取消</button>
            <button class="modal-btn-danger" @click="confirmDelete">确认删除</button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.shelf-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 32px 40px;
  overflow-y: auto;
  user-select: none;
}

.shelf-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.header-left {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.page-title {
  font-size: 26px;
  font-weight: 700;
  color: #f8fafc;
  letter-spacing: -0.02em;
}

.book-count-badge {
  font-size: 13px;
  color: #94a3b8;
  background: #1e293b;
  padding: 2px 10px;
  border-radius: 12px;
  border: 1px solid #334155;
}

.hidden-file-input {
  display: none;
}

.hero-empty-section {
  margin-bottom: 32px;
}

.empty-shelf-card {
  display: flex;
  align-items: center;
  gap: 24px;
  background: linear-gradient(135deg, rgba(30, 41, 59, 0.7) 0%, rgba(15, 23, 42, 0.9) 100%);
  border: 1px dashed #475569;
  border-radius: 16px;
  padding: 32px 36px;
}

.empty-shelf-icon-wrap {
  width: 64px;
  height: 64px;
  border-radius: 14px;
  background: rgba(56, 189, 248, 0.1);
  border: 1px solid rgba(56, 189, 248, 0.25);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.empty-icon {
  font-size: 32px;
}

.empty-shelf-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.empty-shelf-title {
  font-size: 20px;
  font-weight: 700;
  color: #f8fafc;
  margin: 0;
}

.empty-shelf-desc {
  font-size: 13px;
  color: #94a3b8;
  line-height: 1.6;
  margin: 0;
}

.empty-import-action-btn {
  align-self: flex-start;
  margin-top: 6px;
  background: #3b82f6;
  color: #ffffff;
  border: none;
  border-radius: 8px;
  padding: 8px 18px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
}

.empty-import-action-btn:hover {
  background: #2563eb;
  transform: translateY(-1px);
}

.hero-section {
  margin-bottom: 32px;
}

.hero-card {
  display: flex;
  gap: 28px;
  background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%);
  border: 1px solid #334155;
  border-radius: 16px;
  padding: 24px;
  box-shadow: 0 12px 30px -10px rgba(0, 0, 0, 0.4);
}

.hero-cover {
  width: 130px;
  height: 180px;
  border-radius: 10px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.5);
  flex-shrink: 0;
}

.hero-cover-inner {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.hero-cover-char {
  font-size: 40px;
  font-weight: 800;
  color: rgba(255, 255, 255, 0.9);
}

.hero-details {
  display: flex;
  flex-direction: column;
  justify-content: center;
  flex: 1;
}

.hero-label-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}

.hero-badge {
  background: #3b82f6;
  color: white;
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 4px;
}

.hero-timestamp {
  font-size: 12px;
  color: #64748b;
}

.hero-title {
  font-size: 22px;
  font-weight: 700;
  color: #f8fafc;
  margin-bottom: 4px;
}

.hero-author {
  font-size: 14px;
  color: #94a3b8;
  margin-bottom: 8px;
}

.hero-desc {
  font-size: 13px;
  line-height: 1.6;
  color: #94a3b8;
  margin-bottom: 16px;
  white-space: pre-line;
}

.hero-progress-block {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 20px;
  max-width: 480px;
}

.progress-bar-wrap {
  flex: 1;
  height: 6px;
  background: #334155;
  border-radius: 3px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #3b82f6, #38bdf8);
  border-radius: 3px;
  transition: width 0.3s;
}

.progress-text {
  font-size: 12px;
  color: #cbd5e1;
  font-weight: 600;
}

.hero-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.primary-read-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: #f59e0b;
  color: #0f172a;
  border: none;
  border-radius: 8px;
  padding: 10px 22px;
  font-size: 14px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 14px rgba(245, 158, 11, 0.3);
}

.primary-read-btn:hover {
  background: #d97706;
  transform: translateY(-1px);
}

.secondary-audio-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: #1e293b;
  border: 1px solid #475569;
  color: #e2e8f0;
  border-radius: 8px;
  padding: 10px 20px;
  font-size: 14px;
  font-weight: 600;
  line-height: 1;
  cursor: pointer;
  transition: all 0.2s;
}

.secondary-audio-btn:hover {
  background: #334155;
  border-color: #38bdf8;
  color: #38bdf8;
}

.hero-delete-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: #1e293b;
  border: 1px solid #475569;
  color: #94a3b8;
  border-radius: 8px;
  padding: 10px 18px;
  font-size: 14px;
  font-weight: 600;
  line-height: 1;
  cursor: pointer;
  transition: all 0.2s;
}

.hero-delete-btn:hover {
  background: rgba(239, 68, 68, 0.15);
  border-color: #ef4444;
  color: #ef4444;
}

.btn-svg-icon {
  flex-shrink: 0;
  display: block;
}

.shelf-divider {
  height: 1px;
  background: linear-gradient(90deg, rgba(51, 65, 85, 0.6) 0%, rgba(51, 65, 85, 0.15) 100%);
  margin: 28px 0 24px;
  border: none;
}

.drop-hint {
  font-size: 12px;
  color: #64748b;
}

.books-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 24px;
  padding-bottom: 30px;
}

.book-card {
  display: flex;
  flex-direction: column;
  min-height: 320px;
  height: 100%;
  box-sizing: border-box;
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 12px;
  overflow: hidden;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.book-card:hover {
  transform: translateY(-4px);
  border-color: #38bdf8;
  box-shadow: 0 10px 25px -5px rgba(56, 189, 248, 0.2);
}

.card-cover {
  position: relative;
  height: 190px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  gap: 6px;
  overflow: hidden;
}

.card-delete-btn {
  position: absolute;
  top: 10px;
  right: 10px;
  width: 28px;
  height: 28px;
  padding: 0;
  border-radius: 6px;
  background: rgba(15, 23, 42, 0.75);
  border: 1px solid rgba(239, 68, 68, 0.4);
  color: #fca5a5;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  z-index: 10;
  opacity: 0;
  transform: scale(0.85);
  transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  backdrop-filter: blur(4px);
  line-height: 1;
}

.book-card:hover .card-delete-btn {
  opacity: 1;
  transform: scale(1);
}

.card-delete-btn:hover {
  background: #dc2626;
  border-color: #ef4444;
  color: #ffffff;
  transform: scale(1.08);
  box-shadow: 0 4px 12px rgba(220, 38, 38, 0.4);
}

.card-delete-svg {
  display: block;
  flex-shrink: 0;
}

.card-cover-title {
  font-size: 18px;
  font-weight: 700;
  color: white;
  line-height: 1.3;
  z-index: 2;
  text-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
}

.card-cover-desc {
  font-size: 11px;
  line-height: 1.45;
  color: rgba(226, 232, 240, 0.72);
  margin-top: 6px;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  text-overflow: ellipsis;
  z-index: 2;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
  pointer-events: none;
}

.card-glow {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  background: radial-gradient(circle at top right, rgba(255, 255, 255, 0.15), transparent 70%);
}

.card-meta {
  flex: 1;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
}

.card-title {
  font-size: 14px;
  font-weight: 600;
  color: #f1f5f9;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-bottom: 2px;
}

.card-author {
  font-size: 12px;
  color: #94a3b8;
  margin-bottom: 10px;
}

.card-progress-row {
  margin-top: auto;
  padding-top: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.card-progress-track {
  flex: 1;
  height: 4px;
  background: #334155;
  border-radius: 2px;
  overflow: hidden;
}

.card-progress-thumb {
  height: 100%;
  background: #3b82f6;
  border-radius: 2px;
}

.card-progress-num {
  font-size: 11px;
  color: #64748b;
  font-weight: 600;
}

.add-card {
  min-height: 320px;
  height: 100%;
  box-sizing: border-box;
  border: 2px dashed #334155;
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  cursor: pointer;
  transition: all 0.2s;
  background: rgba(30, 41, 59, 0.4);
}

.add-card:hover {
  border-color: #38bdf8;
  background: rgba(56, 189, 248, 0.05);
}

.add-icon {
  font-size: 32px;
  color: #38bdf8;
}

.add-label {
  font-size: 14px;
  font-weight: 600;
  color: #e2e8f0;
}

.shelf-toast {
  position: fixed;
  top: 24px;
  right: 28px;
  background: #0284c7;
  color: #ffffff;
  padding: 10px 18px;
  border-radius: 8px;
  box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.5), 0 0 15px rgba(14, 165, 233, 0.4);
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  font-weight: 600;
  z-index: 1000;
  pointer-events: none;
}

.toast-icon {
  font-size: 16px;
}

.toast-fade-enter-active,
.toast-fade-leave-active {
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.toast-fade-enter-from,
.toast-fade-leave-to {
  opacity: 0;
  transform: translateY(-12px) scale(0.96);
}

/* Import Loading Overlay & Spinner */
.import-loading-overlay {
  position: fixed;
  inset: 0;
  background: rgba(10, 15, 29, 0.85);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 99999;
  pointer-events: all;
  user-select: none;
}

.import-spinner-card {
  background: #1e293b;
  border: 1px solid #38bdf8;
  border-radius: 16px;
  padding: 36px 40px;
  width: 480px;
  max-width: 90vw;
  display: flex;
  flex-direction: column;
  align-items: center;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.7), 0 0 35px rgba(56, 189, 248, 0.25);
  text-align: center;
  animation: dialogPop 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.spinner-ring {
  width: 54px;
  height: 54px;
  border: 4px solid rgba(56, 189, 248, 0.2);
  border-top-color: #38bdf8;
  border-radius: 50%;
  animation: spin 0.85s linear infinite;
  margin-bottom: 20px;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.spinner-title {
  font-size: 20px;
  font-weight: 700;
  color: #f8fafc;
  margin-bottom: 6px;
}

.spinner-filename {
  font-size: 13px;
  color: #94a3b8;
  margin-bottom: 20px;
  word-break: break-all;
  max-width: 100%;
}

.import-progress-container {
  width: 100%;
  margin-bottom: 20px;
}

.import-progress-track {
  width: 100%;
  height: 8px;
  background: #0f172a;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid #334155;
}

.import-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #0284c7, #38bdf8);
  border-radius: 4px;
  transition: width 0.25s ease-out;
}

.import-progress-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
  font-size: 12px;
}

.import-progress-detail {
  color: #cbd5e1;
}

.import-progress-percent {
  color: #38bdf8;
  font-weight: 700;
  font-family: monospace;
}

.spinner-lock-hint {
  font-size: 12px;
  color: #94a3b8;
  display: flex;
  align-items: center;
  gap: 6px;
  line-height: 1.4;
}

/* Delete Confirmation Modal */
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(10, 15, 29, 0.78);
  backdrop-filter: blur(6px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9998;
  user-select: none;
}

.delete-dialog-card {
  background: #1e293b;
  border: 1px solid #475569;
  border-radius: 16px;
  padding: 28px;
  width: 460px;
  max-width: 90vw;
  box-shadow: 0 20px 45px -10px rgba(0, 0, 0, 0.7);
  animation: dialogPop 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes dialogPop {
  from {
    transform: scale(0.95);
    opacity: 0;
  }
  to {
    transform: scale(1);
    opacity: 1;
  }
}

.delete-dialog-header {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 16px;
}

.delete-warn-icon {
  width: 44px;
  height: 44px;
  border-radius: 10px;
  background: rgba(239, 68, 68, 0.15);
  border: 1px solid rgba(239, 68, 68, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 22px;
  flex-shrink: 0;
}

.delete-header-info {
  display: flex;
  flex-direction: column;
}

.delete-dialog-title {
  font-size: 18px;
  font-weight: 700;
  color: #f8fafc;
  margin: 0;
}

.delete-dialog-sub {
  font-size: 12px;
  color: #94a3b8;
  margin-top: 2px;
}

.delete-dialog-body {
  font-size: 14px;
  color: #e2e8f0;
  line-height: 1.6;
  margin-bottom: 20px;
}

.delete-warning-box {
  margin-top: 12px;
  background: #0f172a;
  border: 1px solid rgba(239, 68, 68, 0.4);
  border-radius: 8px;
  padding: 12px 14px;
  font-size: 12px;
  color: #fca5a5;
}

.warning-box-title {
  font-weight: 700;
  margin-bottom: 6px;
  color: #f87171;
}

.warning-box-list {
  margin: 0;
  padding-left: 18px;
  line-height: 1.6;
}

.delete-dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.modal-btn-cancel {
  background: #334155;
  color: #e2e8f0;
  border: none;
  border-radius: 8px;
  padding: 9px 18px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.modal-btn-cancel:hover {
  background: #475569;
}

.modal-btn-danger {
  background: #dc2626;
  color: #ffffff;
  border: none;
  border-radius: 8px;
  padding: 9px 20px;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 14px rgba(220, 38, 38, 0.35);
}

.modal-btn-danger:hover {
  background: #b91c1c;
  transform: translateY(-1px);
}

/* Vue Transitions */
.overlay-fade-enter-active,
.overlay-fade-leave-active,
.modal-fade-enter-active,
.modal-fade-leave-active {
  transition: opacity 0.25s ease;
}

.overlay-fade-enter-from,
.overlay-fade-leave-to,
.modal-fade-enter-from,
.modal-fade-leave-to {
  opacity: 0;
}
</style>
