<script setup lang="ts">
/** Continuous chapter reading and chapter-local sentence navigation. */
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import {
  activeBook,
  activeSentenceIndex,
  isPlaying,
  hasAudio,
  togglePlay,
  prevSentence,
  nextSentence,
  setBookChapter,
  ensureBookLoaded,
  saveReadingProgress,
  flushReadingProgress,
  failure
} from "../stores/library";
import { contents, paragraphs as getChapterParagraphs } from "../text";
import type { Section } from "../../../../protocol/bindings/Section";

const emit = defineEmits<{
  (e: "back-to-shelf"): void;
}>();

const readerViewportRef = ref<HTMLElement | null>(null);
let disposed = false;
let generation = 0;
let jumping: ReturnType<typeof setTimeout> | null = null;
const fontSize = ref(20);
const isTocVisible = ref(true);
const tocSearchQuery = ref("");
const expandedVolumes = ref<Record<string, boolean>>({});
const settings = ref(false);
const controls = ref<HTMLElement | null>(null);
const panel = ref<HTMLElement | null>(null);
const trigger = ref<HTMLButtonElement | null>(null);

/** Open reading controls without keeping keyboard focus behind the panel. */
async function configure(): Promise<void> {
  settings.value = !settings.value;
  if (settings.value) {
    await nextTick();
    if (!disposed && settings.value) panel.value?.querySelector<HTMLInputElement>("input")?.focus();
  }
}

/** Clicking outside the controls closes the nonmodal settings panel. */
function dismiss(event: PointerEvent): void {
  if (settings.value && !controls.value?.contains(event.target as Node)) settings.value = false;
}

const renderedChapters = ref<Section[]>([]);
const isAppending = ref(false);
const isPrepending = ref(false);
const isJumping = ref(false);
/** Shared reading-line distance from the viewport top, in CSS pixels. */
const inset = 100;

/** Associate the reading line with a paragraph, including gaps and trailing chapter space. */
function locate(paragraphs: NodeListOf<HTMLElement>, position: number): number {
  for (let index = 0; index < paragraphs.length; index++) {
    if (paragraphs[index].getBoundingClientRect().bottom >= position) return index;
  }
  return Math.max(0, paragraphs.length - 1);
}

/** Restore the captured reading line; keep the paragraph authoritative after layout changes. */
function restoreReadingPosition(): void {
  const el = readerViewportRef.value;
  if (!el || !activeBook.value) return;

  const targetParaIndex = activeBook.value.currentParagraphIndex ?? 0;
  const targetScrollRatio = activeBook.value.currentScrollRatio ?? 0;
  if (targetParaIndex === 0 && targetScrollRatio === 0) {
    el.scrollTop = 0;
    return;
  }
  const block = el.querySelector<HTMLElement>(
    `.chapter-article-block[data-chapter-index="${activeBook.value.currentChapterIndex}"]`
  );
  if (!block) {
    el.scrollTop = 0;
    return;
  }
  const bounds = block.getBoundingClientRect();
  const paragraphs = block.querySelectorAll<HTMLElement>(".reader-para");
  const target = Math.max(0, Math.min(targetParaIndex, paragraphs.length - 1));
  let position = bounds.top + bounds.height * targetScrollRatio;
  if (paragraphs[target] && (targetScrollRatio === 0 || locate(paragraphs, position) !== target)) {
    const paragraph = paragraphs[target].getBoundingClientRect();
    // Stay inside the target even when the previous paragraph ends exactly at its top.
    position = paragraph.top + Math.min(1, paragraph.height / 2);
  }
  el.scrollTop = Math.max(0, el.scrollTop + position - (el.getBoundingClientRect().top + inset));
}

function initRenderedChapters(): void {
  const token = ++generation;
  if (jumping !== null) clearTimeout(jumping);
  jumping = null;
  isJumping.value = false;
  isAppending.value = false;
  isPrepending.value = false;
  if (!activeBook.value || !activeBook.value.sections || activeBook.value.sections.length === 0) {
    renderedChapters.value = [];
    return;
  }
  const curIdx = activeBook.value.currentChapterIndex ?? 0;
  const initialSec = activeBook.value.sections[curIdx] || activeBook.value.sections[0];
  renderedChapters.value = [initialSec];
  nextTick(() => {
    if (disposed || generation !== token) return;
    restoreReadingPosition();
    const activeTocRow = document.querySelector<HTMLElement>(".chapter-row.active");
    if (activeTocRow) {
      activeTocRow.scrollIntoView({ block: "nearest" });
    }
  });
}

watch(
  () => activeBook.value?.id,
  async (newId) => {
    ++generation;
    if (newId) {
      try { await ensureBookLoaded(newId); } catch { return; }
      if (disposed || activeBook.value?.id !== newId) return;
      initRenderedChapters();
    }
  }
);

function selectChapterSentence(sec: Section, chunkIndex: number): void {
  if (activeBook.value && activeBook.value.currentChapterIndex !== sec.index) {
    void setBookChapter(activeBook.value.id, sec.index);
  }
  activeSentenceIndex.value = chunkIndex;
}

function toggleVolume(volumeName: string): void {
  expandedVolumes.value[volumeName] = !isVolumeExpanded(volumeName);
}

function isVolumeExpanded(volumeName: string): boolean {
  return expandedVolumes.value[volumeName] ?? true;
}

async function onSelectChapter(index: number): Promise<void> {
  const book = activeBook.value;
  if (disposed || !book?.sections) return;
  const sections = book.sections;
  if (!sections[index]) return;

  const token = ++generation;
  if (jumping !== null) clearTimeout(jumping);
  jumping = null;
  isAppending.value = false;
  isPrepending.value = false;
  isJumping.value = true;
  try { await setBookChapter(book.id, index); }
  catch { if (generation === token) isJumping.value = false; return; }
  if (disposed || generation !== token || activeBook.value !== book) return;
  saveReadingProgress(book.id, index, 0, 0);

  // Directly render the target chapter at scrollTop = 0 to guarantee exact chapter display
  renderedChapters.value = [sections[index]];
  nextTick(() => {
    if (disposed || generation !== token || activeBook.value !== book) return;
    const el = readerViewportRef.value;
    if (el) {
      el.scrollTop = 0;
    }
    const activeTocRow = document.querySelector<HTMLElement>(".chapter-row.active");
    if (activeTocRow) {
      activeTocRow.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
    jumping = setTimeout(() => {
      jumping = null;
      if (!disposed && generation === token) isJumping.value = false;
    }, 50);
  });
}

async function handlePrevChapter(): Promise<void> {
  if (!activeBook.value?.sections) return;
  const prevIdx = (activeBook.value.currentChapterIndex ?? 0) - 1;
  if (prevIdx >= 0) {
    await onSelectChapter(prevIdx);
  }
}

async function handleNextChapter(): Promise<void> {
  if (!activeBook.value?.sections) return;
  const nextIdx = (activeBook.value.currentChapterIndex ?? 0) + 1;
  if (nextIdx < activeBook.value.sections.length) {
    await onSelectChapter(nextIdx);
  }
}

function appendNextChapter(): void {
  if (disposed || isAppending.value || isPrepending.value || !activeBook.value?.sections) return;
  const token = generation;
  const sections = activeBook.value.sections;
  if (renderedChapters.value.length === 0) return;

  const lastRendered = renderedChapters.value[renderedChapters.value.length - 1];
  const nextIdx = lastRendered.index + 1;
  if (nextIdx < sections.length) {
    isAppending.value = true;
    renderedChapters.value.push(sections[nextIdx]);
    nextTick(() => {
      if (disposed || generation !== token) return;
      isAppending.value = false;
      trim();
    });
  }
}

function prependPrevChapter(): void {
  const el = readerViewportRef.value;
  if (disposed || !el || !activeBook.value?.sections || renderedChapters.value.length === 0 || isPrepending.value || isAppending.value) return;
  const token = generation;

  const firstRendered = renderedChapters.value[0];
  const prevIdx = firstRendered.index - 1;
  if (prevIdx >= 0) {
    isPrepending.value = true;
    const oldScrollHeight = el.scrollHeight;
    const oldScrollTop = el.scrollTop;

    renderedChapters.value.unshift(activeBook.value.sections[prevIdx]);

    nextTick(() => {
      if (disposed || generation !== token) return;
      const newScrollHeight = el.scrollHeight;
      const heightDiff = newScrollHeight - oldScrollHeight;
      el.scrollTop = oldScrollTop + heightDiff;
      isPrepending.value = false;
      trim();
    });
  }
}

/** Release chapters beyond the viewport buffer while retaining the visible scroll anchor. */
function trim(): void {
  const viewport = readerViewportRef.value;
  if (!viewport || renderedChapters.value.length <= 5 || isAppending.value || isPrepending.value) return;
  const blocks = Array.from(viewport.querySelectorAll<HTMLElement>(".chapter-article-block"));
  if (blocks.length !== renderedChapters.value.length) return;
  const bounds = viewport.getBoundingClientRect();
  let first = 0;
  let end = blocks.length;
  while (end - first > 5 && blocks[first].getBoundingClientRect().bottom < bounds.top - 700) first++;
  while (end - first > 5 && blocks[end - 1].getBoundingClientRect().top > bounds.bottom + 700) end--;
  if (first === 0 && end === blocks.length) return;
  const anchor = blocks[first];
  const offset = anchor.getBoundingClientRect().top;
  const token = generation;
  isPrepending.value = true;
  renderedChapters.value = renderedChapters.value.slice(first, end);
  nextTick(() => {
    if (disposed || generation !== token) return;
    viewport.scrollTop += anchor.getBoundingClientRect().top - offset;
    isPrepending.value = false;
  });
}

function updateReadingPositionFromScroll(): void {
  const el = readerViewportRef.value;
  if (!el || !activeBook.value?.sections || isJumping.value) return;

  const chapterBlocks = el.querySelectorAll<HTMLElement>(".chapter-article-block");
  if (!chapterBlocks || chapterBlocks.length === 0) return;

  const containerRect = el.getBoundingClientRect();
  const detectionLine = containerRect.top + inset;

  for (const block of Array.from(chapterBlocks)) {
    const blockRect = block.getBoundingClientRect();
    if (blockRect.top <= detectionLine && blockRect.bottom >= detectionLine) {
      const idx = block.dataset.chapterIndex;
      if (idx === undefined) break;
      const chapterIdx = parseInt(idx, 10);

      const paragraphs = block.querySelectorAll<HTMLElement>(".reader-para");
      const visibleParaIdx = locate(paragraphs, detectionLine);

      const blockHeight = blockRect.height || 1;
      const blockOffset = Math.max(0, detectionLine - blockRect.top);
      const scrollRatio = Math.min(1.0, Math.max(0.0, blockOffset / blockHeight));

      saveReadingProgress(activeBook.value.id, chapterIdx, visibleParaIdx, scrollRatio);

      if (activeBook.value.currentChapterIndex !== chapterIdx) {
        activeBook.value.currentChapterIndex = chapterIdx;
        const sec = activeBook.value.sections[chapterIdx];
        if (sec) {
          activeBook.value.currentChapter = sec.title;
        }
      }
      break;
    }
  }
}

function handleViewportScroll(): void {
  if (disposed || isJumping.value || isAppending.value || isPrepending.value) return;
  const el = readerViewportRef.value;
  if (!el || !activeBook.value?.sections) return;

  // 1. Auto-stitch next chapter when nearing bottom (within 700px)
  const scrollBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  if (scrollBottom < 700 && !isAppending.value) {
    appendNextChapter();
  }

  // 2. Auto-stitch previous chapter when nearing top (within 200px) with height compensation
  if (el.scrollTop < 200 && !isPrepending.value) {
    prependPrevChapter();
  }

  // 3. Scroll spy to sync active chapter & paragraph position
  updateReadingPositionFromScroll();
  trim();
}

const isBookFinished = computed(() => {
  const sections = activeBook.value?.sections;
  if (!sections || sections.length === 0 || renderedChapters.value.length === 0) return false;
  const lastRendered = renderedChapters.value[renderedChapters.value.length - 1];
  return lastRendered.index >= sections.length - 1;
});

const totalChaptersCount = computed(() => {
  return activeBook.value?.sections?.length ?? activeBook.value?.chapters ?? 0;
});

const totalSentenceCount = computed(() => {
  const b = activeBook.value;
  if (!b) return 0;
  const sec = b.sections?.[b.currentChapterIndex ?? 0];
  if (sec) return contents(sec).length;
  return b.sentences?.length || 0;
});

const filteredVolumeGroups = computed(() => {
  const sections = activeBook.value?.sections || [];
  const query = tocSearchQuery.value.trim().toLowerCase();
  const map = new Map<string, Section[]>();
  const order: string[] = [];

  for (const sec of sections) {
    const vol = sec.volume || "正文";
    if (!map.has(vol)) {
      map.set(vol, []);
      order.push(vol);
    }
    if (!query || sec.title.toLowerCase().includes(query) || vol.toLowerCase().includes(query)) {
      map.get(vol)!.push(sec);
    }
  }

  return order
    .map((vol) => ({
      name: vol,
      sections: map.get(vol) || []
    }))
    .filter((g) => g.sections.length > 0);
});

function handleKeydown(event: KeyboardEvent): void {
  if (settings.value) {
    if (event.code === "Escape") {
      event.preventDefault();
      settings.value = false;
      trigger.value?.focus();
    }
    return;
  }
  const target = document.activeElement;
  if (event.altKey || event.ctrlKey || event.metaKey || event.repeat ||
    target?.matches("input, textarea, select, button, a, [contenteditable=true]")) return;
  if (event.code === "Space") {
    if (!hasAudio.value) return;
    event.preventDefault();
    togglePlay();
  } else if (event.code === "ArrowRight") {
    nextSentence();
  } else if (event.code === "ArrowLeft") {
    prevSentence();
  } else if (event.code === "PageUp") {
    event.preventDefault();
    handlePrevChapter();
  } else if (event.code === "PageDown") {
    event.preventDefault();
    handleNextChapter();
  }
}

async function handleBackToShelf(): Promise<void> {
  try {
    await flushReadingProgress();
    if (!disposed) emit("back-to-shelf");
  } catch { /* The shared banner keeps the failed position observable and retryable. */ }
}

onMounted(async () => {
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("pointerdown", dismiss);
  if (activeBook.value) {
    try { await ensureBookLoaded(activeBook.value.id); } catch { return; }
    if (disposed) return;
    initRenderedChapters();
  }
});

onUnmounted(() => {
  disposed = true;
  ++generation;
  if (jumping !== null) clearTimeout(jumping);
  jumping = null;
  void flushReadingProgress().catch(() => { });
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("pointerdown", dismiss);
});
</script>

<template>
  <div v-if="!activeBook" class="reader-empty-wrapper">
    <div class="reader-empty-card">
      <div class="empty-icon-wrap">📖</div>
      <h2 class="empty-reader-title">当前未打开任何书籍</h2>
      <p class="empty-reader-desc">书架中尚未选择书籍，或当前书籍已被删除。</p>
      <button class="back-shelf-btn-primary" @click="handleBackToShelf">
        ← 返回书架选择书籍
      </button>
    </div>
  </div>

  <div v-else class="reader-container">
    <p v-if="failure" role="alert">{{ failure }}</p>
    <!-- Header Toolbar -->
    <header class="reader-header">
      <button class="toolbar-button" aria-label="返回书架" title="返回书架" @click="handleBackToShelf">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m14 6-6 6 6 6M8 12h12" /></svg>
      </button>
      <button
        class="toolbar-button"
        :class="{ active: isTocVisible }"
        :aria-pressed="isTocVisible"
        aria-controls="reader-toc"
        aria-label="打开或关闭章节目录"
        :title="isTocVisible ? '关闭章节目录' : '打开章节目录'"
        @click="isTocVisible = !isTocVisible"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01" /></svg>
      </button>
      <div ref="controls" class="reading-controls">
        <button
          ref="trigger"
          class="toolbar-button"
          :class="{ active: settings }"
          aria-label="阅读设置"
          title="阅读设置"
          aria-haspopup="dialog"
          aria-controls="reading-settings"
          :aria-expanded="settings"
          @click="configure"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h6m4 0h6M4 17h10m4 0h2M10 4v6M14 14v6" /></svg>
        </button>
        <div v-if="settings" id="reading-settings" ref="panel" class="settings-panel" role="dialog" aria-labelledby="reading-settings-title">
          <h2 id="reading-settings-title">阅读设置</h2>
          <label class="settings-label" for="reading-size">字号 <output>{{ fontSize }} px</output></label>
          <div class="font-stepper">
            <button class="font-btn" :disabled="fontSize <= 16" aria-label="缩小字号" @click="fontSize = Math.max(16, fontSize - 2)">A−</button>
            <input id="reading-size" v-model.number="fontSize" type="range" min="16" max="32" step="2" />
            <button class="font-btn" :disabled="fontSize >= 32" aria-label="放大字号" @click="fontSize = Math.min(32, fontSize + 2)">A+</button>
          </div>
        </div>
      </div>
    </header>

    <!-- Main Workspace: Left Column 2 (TOC Tree) + Center Reading Viewport -->
    <div class="reader-workspace">
      <!-- Left 2nd Column: Volume & Chapter Tree Navigation -->
      <aside id="reader-toc" class="toc-sidebar" :class="{ collapsed: !isTocVisible }" :inert="!isTocVisible">
        <div class="toc-sidebar-inner">
          <div class="toc-header-row">
            <div class="toc-title-box">
              <span class="toc-badge-icon">📑</span>
              <span class="toc-panel-title">卷章树形目录</span>
            </div>
            <span class="toc-count-pill">{{ totalChaptersCount }} 章节</span>
          </div>

          <div class="toc-search-box">
            <span class="toc-search-icon">🔍</span>
            <input
              v-model="tocSearchQuery"
              type="text"
              placeholder="搜索卷名、章节..."
              class="toc-search-input"
            />
          </div>

          <div class="toc-tree-scroll">
            <div
              v-for="group in filteredVolumeGroups"
              :key="group.name"
              class="volume-node"
            >
              <!-- Volume Group Header -->
              <div class="volume-header" @click="toggleVolume(group.name)">
                <span class="volume-arrow">{{ isVolumeExpanded(group.name) ? "▾" : "▸" }}</span>
                <span class="volume-folder-icon">📁</span>
                <span class="volume-name" :title="group.name">{{ group.name }}</span>
                <span class="volume-size">{{ group.sections.length }}</span>
              </div>

              <!-- Chapter Leaves Under Volume -->
              <div v-show="isVolumeExpanded(group.name)" class="chapter-list">
                <div
                  v-for="sec in group.sections"
                  :key="sec.id"
                  class="chapter-row"
                  :class="{ active: activeBook.currentChapterIndex === sec.index }"
                  @click="onSelectChapter(sec.index)"
                >
                  <span class="chapter-indicator"></span>
                  <span class="chapter-row-title" :title="sec.title">{{ sec.title }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </aside>

      <!-- Reading Viewport (Preserving Paragraphs & Sentence Highlights with Continuous Stitching) -->
      <main ref="readerViewportRef" class="reader-viewport" @scroll="handleViewportScroll">
        <div class="reader-card">
          <!-- Continuous Rendered Chapters Stream -->
          <article
            v-for="(chapter, cIdx) in renderedChapters"
            :key="chapter.id"
            class="chapter-article-block"
            :data-chapter-index="chapter.index"
          >
            <h2 class="article-chapter-title" :class="{ 'stitched-title': cIdx > 0 }">
              {{ chapter.title }}
            </h2>

            <div class="article-body" :style="{ fontSize: `${fontSize}px` }">
              <p
                v-for="para in getChapterParagraphs(chapter)"
                :key="para.pIndex"
                class="reader-para"
                :data-para-index="para.pIndex"
              >
                <span
                  v-for="chunk in para.chunks"
                  :key="chunk.id"
                  class="saga-chunk"
                  :class="{ active: activeBook.currentChapterIndex === chapter.index && activeSentenceIndex === chunk.index }"
                  @click="selectChapterSentence(chapter, chunk.index)"
                >
                  {{ chunk.text }}
                </span>
              </p>
            </div>
          </article>


          <!-- Book Completion Indicator -->
          <div v-if="isBookFinished" class="book-finished-indicator">
            <div class="finished-divider"></div>
            <div class="finished-badge">
              <span class="finished-icon">🎉</span>
              <span class="finished-title">全书已完结</span>
              <span class="finished-desc">已读完全部 {{ totalChaptersCount }} 个章节</span>
            </div>
          </div>
        </div>
      </main>
    </div>

    <!-- Bottom Controls & Shortcut Hints -->
    <footer v-if="hasAudio" class="reader-footer">
      <div class="controls">
        <button class="play-btn" :class="{ playing: isPlaying }" @click="togglePlay">
          {{ isPlaying ? "⏸ 暂停伴读 (Space)" : "▶ 开始伴读 (Space)" }}
        </button>
        <span class="progress-indicator">
          句子 {{ activeSentenceIndex + 1 }} / {{ totalSentenceCount }}
        </span>
      </div>
      <div class="hotkeys-hint">
        <span>快捷键: [空格] 播放/暂停</span>
        <span>[← / →] 逐句切片</span>
        <span>[PageUp / PageDn] 切换章节</span>
        <span>[点击句子] 直达朗读</span>
      </div>
    </footer>

  </div>
</template>

<style scoped>
.reader-empty-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 40px;
  background: #0f172a;
  user-select: none;
}

.reader-empty-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 16px;
  padding: 48px 40px;
  max-width: 420px;
  box-shadow: 0 16px 36px -10px rgba(0, 0, 0, 0.5);
}

.empty-icon-wrap {
  font-size: 48px;
  margin-bottom: 16px;
}

.empty-reader-title {
  font-size: 20px;
  font-weight: 700;
  color: #f8fafc;
  margin: 0 0 8px 0;
}

.empty-reader-desc {
  font-size: 14px;
  color: #94a3b8;
  margin: 0 0 24px 0;
  line-height: 1.5;
}

.back-shelf-btn-primary {
  background: #3b82f6;
  color: white;
  border: none;
  border-radius: 8px;
  padding: 10px 24px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 14px rgba(59, 130, 246, 0.35);
}

.back-shelf-btn-primary:hover {
  background: #2563eb;
  transform: translateY(-1px);
}

.reader-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #0f172a;
  color: #f1f5f9;
  user-select: none;
  overflow: hidden;
}

.reader-header {
  display: flex;
  align-items: center;
  padding: 10px 24px;
  background: #1e293b;
  border-bottom: 1px solid #334155;
  flex-shrink: 0;
  gap: 8px;
  z-index: 20;
}

.toolbar-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  padding: 9px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 8px;
  color: #cbd5e1;
  cursor: pointer;
  transition: color 0.15s, background 0.15s;
}

.toolbar-button svg {
  width: 22px;
  height: 22px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.toolbar-button:hover,
.toolbar-button.active {
  color: #7dd3fc;
  background: rgba(56, 189, 248, 0.12);
}

.toolbar-button:focus-visible,
.font-btn:focus-visible {
  outline: 2px solid #38bdf8;
  outline-offset: 2px;
}

.reading-controls {
  position: relative;
  margin-left: auto;
}

.settings-panel {
  position: absolute;
  top: calc(100% + 10px);
  right: 0;
  width: min(280px, calc(100vw - 72px));
  padding: 20px;
  border: 1px solid #475569;
  border-radius: 12px;
  background: #1e293b;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
}

.settings-panel h2 {
  margin: 0 0 20px;
  color: #f1f5f9;
  font-size: 15px;
}

.settings-label {
  display: flex;
  justify-content: space-between;
  margin-bottom: 12px;
  color: #cbd5e1;
  font-size: 13px;
}

.settings-label output {
  color: #7dd3fc;
  font-variant-numeric: tabular-nums;
}

.font-stepper {
  display: flex;
  align-items: center;
  gap: 12px;
}

.font-stepper input {
  min-width: 0;
  flex: 1;
  accent-color: #38bdf8;
}

.font-btn {
  padding: 6px;
  border: none;
  border-radius: 4px;
  background: #334155;
  color: #e2e8f0;
  cursor: pointer;
}

.font-btn:disabled {
  opacity: 0.4;
  cursor: default;
}
/* 2-Column Reader Workspace */
.reader-workspace {
  flex: 1;
  display: flex;
  min-height: 0;
  position: relative;
  overflow: hidden;
}

/* Left Column 2: Volume & Chapter Tree Sidebar */
.toc-sidebar {
  width: 280px;
  background: #162032;
  border-right: 1px solid #334155;
  display: flex;
  flex-direction: column;
  transition: width 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  flex-shrink: 0;
}

.toc-sidebar.collapsed {
  width: 0;
  border-right: none;
}

.toc-sidebar-inner {
  width: 280px;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.toc-header-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 16px 10px;
}

.toc-title-box {
  display: flex;
  align-items: center;
  gap: 6px;
}

.toc-badge-icon {
  font-size: 15px;
}

.toc-panel-title {
  font-size: 14px;
  font-weight: 700;
  color: #f1f5f9;
}

.toc-count-pill {
  font-size: 11px;
  color: #94a3b8;
  background: #1e293b;
  padding: 2px 8px;
  border-radius: 10px;
  border: 1px solid #334155;
}

.toc-search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 14px 10px;
  padding: 6px 10px;
  background: #0f172a;
  border: 1px solid #334155;
  border-radius: 6px;
}

.toc-search-icon {
  font-size: 12px;
  color: #64748b;
}

.toc-search-input {
  background: transparent;
  border: none;
  outline: none;
  color: #f1f5f9;
  font-size: 12px;
  width: 100%;
}

.toc-search-input::placeholder {
  color: #64748b;
}

.toc-tree-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px 16px;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.toc-tree-scroll::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.volume-node {
  margin-bottom: 6px;
}

.volume-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  background: rgba(30, 41, 59, 0.6);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  user-select: none;
}

.volume-header:hover {
  background: #1e293b;
  color: #38bdf8;
}

.volume-arrow {
  font-size: 12px;
  color: #94a3b8;
  width: 12px;
}

.volume-folder-icon {
  font-size: 13px;
}

.volume-name {
  font-size: 13px;
  font-weight: 600;
  color: #e2e8f0;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.volume-size {
  font-size: 11px;
  color: #64748b;
}

.chapter-list {
  display: flex;
  flex-direction: column;
  padding-left: 12px;
  margin-top: 2px;
  gap: 2px;
}

.chapter-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  border-left: 2px solid transparent;
}

.chapter-row:hover {
  background: rgba(30, 41, 59, 0.8);
  color: #38bdf8;
}

.chapter-row.active {
  background: rgba(56, 189, 248, 0.12);
  border-left-color: #38bdf8;
  color: #38bdf8;
  font-weight: 700;
}

.chapter-indicator {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #475569;
  flex-shrink: 0;
}

.chapter-row.active .chapter-indicator {
  background: #38bdf8;
  box-shadow: 0 0 8px #38bdf8;
}

.chapter-row-title {
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Reading Viewport */
.reader-viewport {
  flex: 1;
  display: flex;
  justify-content: center;
  overflow-y: auto;
  padding: 40px 24px;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.reader-viewport::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.reader-card {
  max-width: 820px;
  width: 100%;
  background: #1e293b;
  border: 1px solid #334155;
  border-radius: 12px;
  padding: 44px 56px;
  box-shadow: 0 10px 35px -5px rgba(0, 0, 0, 0.5);
  margin-bottom: 40px;
  align-self: flex-start;
}

.article-chapter-title {
  font-size: 24px;
  font-weight: 800;
  color: #f8fafc;
  margin-bottom: 32px;
  padding-bottom: 16px;
  border-bottom: 1px solid #334155;
  letter-spacing: -0.01em;
}

.article-body {
  letter-spacing: 0.04em;
  color: #e2e8f0;
}

/* Authentic Chinese 2-character indentation & paragraph separation */
.reader-para {
  text-indent: 2em;
  margin-bottom: 1.3em;
  line-height: 2.1;
  text-align: justify;
}

.saga-chunk {
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
  transition: all 0.2s ease;
}

.saga-chunk:hover {
  background: rgba(56, 189, 248, 0.2);
}

.saga-chunk.active {
  background: #f59e0b;
  color: #0f172a;
  font-weight: 700;
  box-shadow: 0 0 14px rgba(245, 158, 11, 0.5);
}

/* Chapter Continuous Stitching */
.chapter-article-block {
  margin-bottom: 36px;
}

.stitched-title {
  font-size: 24px;
  color: #f8fafc;
  margin-top: 64px;
  margin-bottom: 24px;
  padding-bottom: 12px;
}

/* Book Finished Indicator */
.book-finished-indicator {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-top: 60px;
  padding-bottom: 20px;
  user-select: none;
}

.finished-divider {
  width: 100%;
  height: 1px;
  background: linear-gradient(90deg, transparent, #475569, transparent);
  margin-bottom: 32px;
}

.finished-badge {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  background: rgba(30, 41, 59, 0.85);
  border: 1px solid #334155;
  border-radius: 16px;
  padding: 24px 40px;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.4);
}

.finished-icon {
  font-size: 32px;
}

.finished-title {
  font-size: 18px;
  font-weight: 700;
  color: #f8fafc;
}

.finished-desc {
  font-size: 13px;
  color: #94a3b8;
}

.reader-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 24px;
  background: #1e293b;
  border-top: 1px solid #334155;
  flex-shrink: 0;
}

.controls {
  display: flex;
  align-items: center;
  gap: 16px;
}

.play-btn {
  background: #3b82f6;
  color: white;
  border: none;
  padding: 8px 18px;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s;
}

.play-btn:hover {
  background: #2563eb;
}

.play-btn.playing {
  background: #f59e0b;
  color: #0f172a;
}

.progress-indicator {
  font-size: 13px;
  color: #94a3b8;
}

.hotkeys-hint {
  display: flex;
  gap: 16px;
  font-size: 12px;
  color: #64748b;
}

</style>
