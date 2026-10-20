<script setup lang="ts">
/** Visual sentence demonstration controls; no audio synthesis is attached. */
import { computed } from "vue";
import {
  activeBook,
  hasAudio,
  activeSentenceIndex,
  isPlaying,
  speechRate,
  togglePlay,
  prevSentence,
  nextSentence
} from "../stores/library";

const emit = defineEmits<{
  (e: "open-reader"): void;
}>();

const currentSentence = computed(() => {
  const sentences = activeBook.value?.sentences || [];
  return sentences[activeSentenceIndex.value] || "";
});

function cycleRate(): void {
  if (speechRate.value === 1.0) speechRate.value = 1.25;
  else if (speechRate.value === 1.25) speechRate.value = 1.5;
  else speechRate.value = 1.0;
}
</script>

<template>
  <div v-if="activeBook && hasAudio" class="mini-bar">
    <div class="book-info" @click="emit('open-reader')">
      <div class="mini-cover" :style="{ background: activeBook.coverGradient }">
        <span>{{ activeBook.title[0] || '书' }}</span>
      </div>
      <div class="text-meta">
        <div class="title-row">
          <span class="book-name">{{ activeBook.title }}</span>
          <span class="chapter-name">{{ activeBook.currentChapter }}</span>
        </div>
        <div class="sentence-snippet">
          <span class="quote">“</span>
          {{ currentSentence }}
          <span class="quote">”</span>
        </div>
      </div>
    </div>

    <div class="playback-controls">
      <button class="step-btn" title="上一句" @click="prevSentence">
        ⏮
      </button>
      <button
        class="play-toggle-btn"
        :class="{ playing: isPlaying }"
        :title="isPlaying ? '暂停 (Space)' : '播放 (Space)'"
        @click="togglePlay"
      >
        {{ isPlaying ? "⏸" : "▶" }}
      </button>
      <button class="step-btn" title="下一句" @click="nextSentence">
        ⏭
      </button>
    </div>

    <div class="utility-actions">
      <button class="rate-badge" title="切换伴读语速" @click="cycleRate">
        {{ speechRate.toFixed(2) }}x
      </button>
      <button class="expand-btn" title="进入全屏阅读器" @click="emit('open-reader')">
        📖 打开阅读器
      </button>
    </div>
  </div>
</template>

<style scoped>
.mini-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 64px;
  background: #0f172a;
  border-top: 1px solid #334155;
  padding: 0 20px;
  box-shadow: 0 -4px 16px rgba(0, 0, 0, 0.35);
  z-index: 50;
  user-select: none;
}

.book-info {
  display: flex;
  align-items: center;
  gap: 12px;
  max-width: 50%;
  cursor: pointer;
  overflow: hidden;
}

.mini-cover {
  width: 36px;
  height: 48px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  color: white;
  font-size: 14px;
  flex-shrink: 0;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
}

.text-meta {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
}

.book-name {
  font-size: 13px;
  font-weight: 600;
  color: #f8fafc;
}

.chapter-name {
  font-size: 11px;
  color: #94a3b8;
}

.sentence-snippet {
  font-size: 12px;
  color: #38bdf8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 2px;
}

.quote {
  color: #f59e0b;
  font-weight: bold;
}

.playback-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

.step-btn {
  background: transparent;
  border: none;
  color: #94a3b8;
  font-size: 14px;
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
  transition: all 0.2s;
}

.step-btn:hover {
  color: #f8fafc;
  background: rgba(255, 255, 255, 0.08);
}

.play-toggle-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: #3b82f6;
  color: white;
  border: none;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: 0 2px 8px rgba(59, 130, 246, 0.4);
  transition: all 0.2s;
}

.play-toggle-btn:hover {
  background: #2563eb;
  transform: scale(1.05);
}

.play-toggle-btn.playing {
  background: #f59e0b;
  box-shadow: 0 0 12px rgba(245, 158, 11, 0.5);
}

.utility-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.rate-badge {
  background: #1e293b;
  border: 1px solid #475569;
  color: #cbd5e1;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.rate-badge:hover {
  border-color: #38bdf8;
  color: #38bdf8;
}

.expand-btn {
  background: #1e293b;
  border: 1px solid #334155;
  color: #f1f5f9;
  font-size: 12px;
  font-weight: 500;
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

.expand-btn:hover {
  background: #334155;
  border-color: #64748b;
}
</style>
