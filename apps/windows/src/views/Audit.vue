<script setup lang="ts">
/** Isolated chapter-edit drafts, retained findings, and exact heading navigation. */
import { ref, shallowRef, triggerRef, computed, watch, onMounted, nextTick } from "vue";
import {
  activeAuditReport,
  activeAuditNovel,
  activeBook,
  saveAuditedSections,
  selectBook
} from "../stores/library";
import type { Section } from "../../../../protocol/bindings/Section";
import type { ValidationAnomaly } from "../../../../protocol/bindings/ValidationAnomaly";

const emit = defineEmits<{
  (e: "open-reader"): void;
  (e: "back-to-shelf"): void;
}>();

interface AnomalyItem extends ValidationAnomaly {
  anomalyId: number;
  initialSectionId: string;
}

interface ResolutionRecord {
  action: "demoted_to_prev" | "demoted_next" | "renamed" | "deleted" | "accepted";
  label: string;
}

interface HistoryStep {
  sectionsSnapshot: Section[];
  resolvedMapSnapshot: Record<number, ResolutionRecord>;
}

const sections = shallowRef<Section[]>([]);
const anomalies = ref<AnomalyItem[]>([]);
const selectedAnomalyIndex = ref<number>(0);
const filterType = ref<string>("PENDING");
const resolvedMap = ref<Record<number, ResolutionRecord>>({});
const historyStack = ref<HistoryStep[]>([]);
const isSaving = ref(false);
const failure = ref('');

const renderedSections = ref<Section[]>([]);
const activePopoverSection = ref<Section | null>(null);
const editTitle = ref("");
const editVolume = ref("");
const readerPaperRef = ref<HTMLElement | null>(null);
const isPrependingAudit = ref(false);
const isAppendingAudit = ref(false);

const bookTitle = computed(() => {
  return activeAuditNovel.value?.book.title || activeBook.value?.title || "未命名小说";
});

const bookAuthor = computed(() => {
  return activeAuditNovel.value?.book.author || activeBook.value?.author || "未知作者";
});

const bookId = computed(() => {
  return activeAuditNovel.value?.book.id || activeBook.value?.id || "";
});

// Copy editable objects; immutable text is shared without serializing the full book.
function clone(source: Section[]): Section[] {
  return source.map(section => ({ ...section, chunks: section.chunks.map(chunk => ({ ...chunk })) }));
}

function syncRenderedWindow(centerIdx: number): void {
  if (sections.value.length === 0) {
    renderedSections.value = [];
    return;
  }
  const start = Math.max(0, centerIdx - 1);
  const end = Math.min(sections.value.length - 1, centerIdx + 2);
  renderedSections.value = sections.value.slice(start, end + 1);
}

function initAuditState(): void {
  const sourceSections =
    activeAuditNovel.value?.sections || activeBook.value?.sections || [];
  // Isolate editable objects while sharing immutable body strings.
  sections.value = clone(sourceSections);

  const sourceAnomalies = activeAuditReport.value?.anomalies || [];
  anomalies.value = sourceAnomalies.map((a, idx) => {
    const sec = sourceSections.find(section => section.index === a.section_index);
    return {
      ...a,
      anomalyId: idx,
      initialSectionId: sec ? sec.id : `${bookId.value}:${a.section_index}`
    };
  });

  resolvedMap.value = {};
  for (const anomaly of anomalies.value) {
    if (anomaly.retained) resolvedMap.value[anomaly.anomalyId] = { action: "accepted", label: "已保留" };
  }
  failure.value = "";
  filterType.value = "PENDING";
  historyStack.value = [];
  selectedAnomalyIndex.value = 0;
  activePopoverSection.value = null;

  const firstAnomaly = anomalies.value.find(anomaly => !anomaly.retained);
  const centerIdx = firstAnomaly ? firstAnomaly.section_index : 0;
  syncRenderedWindow(centerIdx);

  if (firstAnomaly) selectAnomaly(firstAnomaly.anomalyId, false);
}

function recordHistorySnapshot(): void {
  historyStack.value.push({
    sectionsSnapshot: clone(sections.value),
    resolvedMapSnapshot: { ...resolvedMap.value }
  });
  if (historyStack.value.length > 30) {
    historyStack.value.shift();
  }
}

function handleUndo(): void {
  if (historyStack.value.length === 0) return;
  const last = historyStack.value.pop()!;
  sections.value = last.sectionsSnapshot;
  resolvedMap.value = last.resolvedMapSnapshot;

  if (activePopoverSection.value) {
    const found = sections.value.find(s => s.id === activePopoverSection.value!.id);
    if (found) {
      activePopoverSection.value = found;
      editTitle.value = found.title;
      editVolume.value = found.volume;
    }
  }
  const selected = anomalies.value[selectedAnomalyIndex.value];
  const index = sections.value.findIndex(section => section.id === selected?.initialSectionId);
  syncRenderedWindow(Math.max(0, index));
  if (index >= 0) selectAnomaly(selectedAnomalyIndex.value);
}

const filteredAnomalies = computed(() => {
  if (filterType.value === "PENDING") return anomalies.value.filter(item => !resolvedMap.value[item.anomalyId]);
  if (filterType.value === "RESOLVED") return anomalies.value.filter(item => resolvedMap.value[item.anomalyId]);
  if (filterType.value === "ALL") {
    return anomalies.value;
  }
  return anomalies.value.filter((a) => a.anomaly_type === filterType.value);
});

const resolvedCount = computed(() => {
  return Object.keys(resolvedMap.value).length;
});

const totalAnomaliesCount = computed(() => {
  return anomalies.value.length;
});

function appendNextAuditSection(): void {
  if (renderedSections.value.length === 0 || isAppendingAudit.value) return;
  const last = renderedSections.value[renderedSections.value.length - 1];
  const nextIdx = sections.value.findIndex(s => s.id === last.id) + 1;
  if (nextIdx > 0 && nextIdx < sections.value.length) {
    isAppendingAudit.value = true;
    renderedSections.value.push(sections.value[nextIdx]);
    nextTick(() => {
      isAppendingAudit.value = false;
    });
  }
}

function prependPrevAuditSection(): void {
  const el = readerPaperRef.value;
  if (!el || renderedSections.value.length === 0 || isPrependingAudit.value) return;
  const first = renderedSections.value[0];
  const prevIdx = sections.value.findIndex(s => s.id === first.id) - 1;
  if (prevIdx >= 0) {
    isPrependingAudit.value = true;
    const oldScrollHeight = el.scrollHeight;
    const oldScrollTop = el.scrollTop;

    renderedSections.value.unshift(sections.value[prevIdx]);

    nextTick(() => {
      const newScrollHeight = el.scrollHeight;
      el.scrollTop = oldScrollTop + (newScrollHeight - oldScrollHeight);
      isPrependingAudit.value = false;
    });
  }
}

function handleAuditScroll(): void {
  const el = readerPaperRef.value;
  if (!el) return;

  const scrollBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  if (scrollBottom < 600 && !isAppendingAudit.value) {
    appendNextAuditSection();
  }

  if (el.scrollTop < 150 && !isPrependingAudit.value) {
    prependPrevAuditSection();
  }
}

function openPopoverForSection(sec: Section): void {
  activePopoverSection.value = sec;
  editTitle.value = sec.title;
  editVolume.value = sec.volume;

  const anomalyIdx = anomalies.value.findIndex(
    a => a.initialSectionId === sec.id
  );
  if (anomalyIdx !== -1) {
    selectedAnomalyIndex.value = anomalyIdx;
  }
}

function closePopover(): void {
  activePopoverSection.value = null;
}

function selectAnomaly(anomalyIdx: number, openPopup: boolean = false): void {
  selectedAnomalyIndex.value = anomalyIdx;
  const anomaly = anomalies.value[anomalyIdx];
  if (!anomaly) return;

  const secIdx = sections.value.findIndex(s => s.id === anomaly.initialSectionId);
  const targetSec = sections.value[secIdx];
  if (!targetSec) return;

  const inWindow = renderedSections.value.some(s => s.id === targetSec.id);
  if (!inWindow) {
    syncRenderedWindow(secIdx);
  }

  nextTick(() => {
    if (selectedAnomalyIndex.value !== anomalyIdx) return;
    const blocks = readerPaperRef.value?.querySelectorAll<HTMLElement>("[data-section-id]");
    const block = blocks && Array.from(blocks).find(element => element.dataset.sectionId === targetSec.id);
    const heading = block?.querySelector<HTMLElement>("h2, .novel-suspicious-heading-line");
    // A direct jump cannot race the viewport's prepend/append scroll anchoring.
    heading?.scrollIntoView({ behavior: "auto", block: "start" });
    if (openPopup) {
      openPopoverForSection(targetSec);
    }
  });
}

function advanceToNextUnresolved(): void {
  // Refresh shallow section state after an edit, before choosing the next finding.
  triggerRef(sections);
  const selected = anomalies.value[selectedAnomalyIndex.value];
  const position = sections.value.findIndex(section => section.id === selected?.initialSectionId);
  syncRenderedWindow(Math.max(0, position));
  const nextIdx = anomalies.value.findIndex(
    (a, idx) => idx > selectedAnomalyIndex.value && !resolvedMap.value[a.anomalyId]
  );
  if (nextIdx !== -1) {
    selectAnomaly(nextIdx, false);
    return;
  }
  const anyUnresolved = anomalies.value.findIndex(
    (a) => !resolvedMap.value[a.anomalyId]
  );
  if (anyUnresolved !== -1) {
    selectAnomaly(anyUnresolved, false);
  }
}

function handleDemoteToPrev(): void {
  if (!activePopoverSection.value) return;
  const targetId = activePopoverSection.value.id;
  const targetIdx = sections.value.findIndex(s => s.id === targetId);
  if (targetIdx <= 0) return;

  recordHistorySnapshot();

  const prev = sections.value[targetIdx - 1];
  const curr = sections.value[targetIdx];

  const textToAdd = curr.body.trim()
    ? `\n\n${curr.title}\n\n${curr.body}`
    : `\n\n${curr.title}`;
  prev.body += textToAdd;

  if (curr.chunks && curr.chunks.length > 0) {
    const baseP = prev.chunks.length > 0 ? prev.chunks[prev.chunks.length - 1].paragraph + 1 : 0;
    prev.chunks.push(...curr.chunks.map((c, i) => ({
      ...c,
      id: `${prev.id}:p:${i}`,
      chapter: prev.id,
      paragraph: baseP + c.paragraph
    })));
  }

  sections.value.splice(targetIdx, 1);

  const rIdx = renderedSections.value.findIndex(s => s.id === targetId);
  if (rIdx !== -1) {
    renderedSections.value.splice(rIdx, 1);
  }

  const anomaly = anomalies.value.find(a => a.initialSectionId === targetId);
  if (anomaly) {
    resolvedMap.value[anomaly.anomalyId] = {
      action: "demoted_to_prev",
      label: `已变回《${prev.title}》正文段落`
    };
  }

  closePopover();
  advanceToNextUnresolved();
}

function handleDemoteNext(): void {
  if (!activePopoverSection.value) return;
  const targetId = activePopoverSection.value.id;
  const targetIdx = sections.value.findIndex(s => s.id === targetId);
  if (targetIdx < 0 || targetIdx >= sections.value.length - 1) return;

  recordHistorySnapshot();

  const curr = sections.value[targetIdx];
  const next = sections.value[targetIdx + 1];
  const demotedTitle = next.title;

  const addedText = next.body.trim()
    ? `\n\n${next.title}\n\n${next.body}`
    : `\n\n${next.title}`;
  curr.body += addedText;

  if (next.chunks && next.chunks.length > 0) {
    const baseP = curr.chunks.length > 0 ? curr.chunks[curr.chunks.length - 1].paragraph + 1 : 0;
    curr.chunks.push(...next.chunks.map((c, i) => ({
      ...c,
      id: `${curr.id}:next_p:${i}`,
      chapter: curr.id,
      paragraph: baseP + c.paragraph
    })));
  }

  const nextId = next.id;
  sections.value.splice(targetIdx + 1, 1);

  const rNextIdx = renderedSections.value.findIndex(s => s.id === nextId);
  if (rNextIdx !== -1) {
    renderedSections.value.splice(rNextIdx, 1);
  }

  const anomaly = anomalies.value.find(a => a.initialSectionId === nextId);
  if (anomaly) {
    resolvedMap.value[anomaly.anomalyId] = {
      action: "demoted_next",
      label: `已取消伪章节《${demotedTitle}》，正文已接续归入本章`
    };
  }

  closePopover();
  advanceToNextUnresolved();
}

function handleApplyRename(): void {
  if (!activePopoverSection.value || !editTitle.value.trim()) return;
  recordHistorySnapshot();

  const sec = activePopoverSection.value;
  sec.title = editTitle.value.trim();
  if (editVolume.value.trim()) {
    sec.volume = editVolume.value.trim();
  }

  const found = sections.value.find(s => s.id === sec.id);
  if (found) {
    found.title = sec.title;
    found.volume = sec.volume;
  }

  const anomaly = anomalies.value.find(a => a.initialSectionId === sec.id);
  if (anomaly) {
    resolvedMap.value[anomaly.anomalyId] = {
      action: "renamed",
      label: `已更名为《${sec.title}》`
    };
  }

  closePopover();
  advanceToNextUnresolved();
}

function handleDeleteSection(): void {
  if (!activePopoverSection.value) return;
  const targetId = activePopoverSection.value.id;
  const targetIdx = sections.value.findIndex(s => s.id === targetId);
  if (targetIdx < 0) return;

  recordHistorySnapshot();

  const deletedTitle = activePopoverSection.value.title;
  sections.value.splice(targetIdx, 1);

  const rIdx = renderedSections.value.findIndex(s => s.id === targetId);
  if (rIdx !== -1) {
    renderedSections.value.splice(rIdx, 1);
  }

  const anomaly = anomalies.value.find(a => a.initialSectionId === targetId);
  if (anomaly) {
    resolvedMap.value[anomaly.anomalyId] = {
      action: "deleted",
      label: `已删除章节《${deletedTitle}》`
    };
  }

  closePopover();
  advanceToNextUnresolved();
}

function retain(): void {
  if (!activePopoverSection.value) return;
  const sec = activePopoverSection.value;
  recordHistorySnapshot();

  for (const anomaly of anomalies.value.filter(a => a.initialSectionId === sec.id)) {
    resolvedMap.value[anomaly.anomalyId] = {
      action: "accepted",
      label: "已保留"
    };
  }

  closePopover();
  advanceToNextUnresolved();
}

function restore(): void {
  if (!activePopoverSection.value) return;
  recordHistorySnapshot();
  for (const anomaly of anomalies.value.filter(item => item.initialSectionId === activePopoverSection.value?.id)) {
    if (resolvedMap.value[anomaly.anomalyId]?.action === "accepted") delete resolvedMap.value[anomaly.anomalyId];
  }
  closePopover();
}

function retained(section: Section): boolean {
  return anomalies.value.some(item => item.initialSectionId === section.id && resolvedMap.value[item.anomalyId]?.action === "accepted");
}

async function saveAndOpenReader(): Promise<void> {
  if (isSaving.value) return;
  const identity = bookId.value;
  isSaving.value = true;
  failure.value = "";

  try {
    const normalized = clone(sections.value);
    const retained = anomalies.value.filter(item => resolvedMap.value[item.anomalyId]?.action === "accepted").map(item => item.key);
    normalized.forEach((sec, idx) => {
      sec.index = idx;
      sec.id = `${identity}:${idx}`;
      sec.chunks.forEach((c, cIdx) => {
        c.chapter = sec.id;
        c.index = cIdx;
      });
    });

    await saveAuditedSections(identity, normalized, retained);
    activeAuditReport.value = null;
    activeAuditNovel.value = null;

    selectBook(identity);
    emit("open-reader");
  } catch (err) {
    failure.value = `保存失败，修改仍留在当前页面，请重试：${err}`;
    console.error("Save audited sections failed:", err);
  } finally {
    isSaving.value = false;
  }
}

function skipAndOpenReader(): void {
  const identity = bookId.value;
  activeAuditReport.value = null;
  activeAuditNovel.value = null;
  selectBook(identity);
  emit("open-reader");
}

function formatAnomalyTag(type: string): string {
  switch (type) {
    case "SuspiciousShortSection":
      return "短正文";
    case "SequenceGap":
      return "序号跳跃";
    case "SequenceRegression":
      return "序号逆转";
    case "DuplicateHeading":
      return "重复章节";
    case "VolumeDisorder":
      return "卷序异常";
    default:
      return "可疑章节";
  }
}

function getAnomalyExplanation(anomaly: AnomalyItem | ValidationAnomaly): {
  tag: string;
  tip: string;
} {
  switch (anomaly.anomaly_type) {
    case "SuspiciousShortSection":
      return {
        tag: "可疑短正文（正文极短或为0）",
        tip: `本节正文极短或为0字（${anomaly.description}）。常见原因：正文时间句（如『3.5小时后……』）或对话被误切为独立章节，导致前一章被提前截断。`,
      };
    case "SequenceGap":
      return {
        tag: "章节序号跳跃",
        tip: `章节序号出现跨越跳号（${anomaly.description}）。常见于作者连载手误跳号或缺章。`,
      };
    case "SequenceRegression":
      return {
        tag: "章节序号逆转",
        tip: `章节编号倒退减小（${anomaly.description}）。可能是新卷开启未标卷名，或正文提及旧章节标题。`,
      };
    case "DuplicateHeading":
      return {
        tag: "章号或标题重复",
        tip: `${anomaly.description}。重复章号不代表内容重复；如果原文如此，可选择“保留”，阅读顺序保持不变。`,
      };
    case "VolumeDisorder":
      return {
        tag: "分卷结构异常",
        tip: `分卷出现不连续或乱序（${anomaly.description}）。`,
      };
    default:
      return {
        tag: "可疑条目",
        tip: anomaly.description,
      };
  }
}

function getSectionParas(sec: Section): string[] {
  if (!sec.body) return [];
  return sec.body.split(/\r?\n/).map(p => p.trim()).filter(p => p.length > 0);
}

function isAnomalySection(sec: Section): boolean {
  return anomalies.value.some(
    a => (a.initialSectionId === sec.id) && !resolvedMap.value[a.anomalyId]
  );
}

function isSectionResolved(sec: Section): boolean {
  return anomalies.value.some(
    a => (a.initialSectionId === sec.id) && !!resolvedMap.value[a.anomalyId]
  );
}

function getPopoverAnomaly(sec: Section | null): { tag: string; tip: string } | null {
  if (!sec) return null;
  const a = anomalies.value.find(item => item.initialSectionId === sec.id);
  if (!a) {
    return {
      tag: "正文章节",
      tip: `当前章节字数 ${sec.body.length} 字。您可在此编辑更名或取消切分。`
    };
  }
  const exp = getAnomalyExplanation(a);
  return {
    tag: exp.tag,
    tip: exp.tip
  };
}

function getSectionIndex(sec: Section | null): number {
  if (!sec) return -1;
  return sections.value.findIndex(s => s.id === sec.id);
}

onMounted(() => {
  initAuditState();
});

watch(
  () => activeAuditReport.value,
  () => {
    initAuditState();
  }
);
</script>

<template>
  <div class="audit-page">
    <!-- Top Action Navigation Header -->
    <header class="audit-header">
      <div class="header-left">
        <button class="back-shelf-btn" :disabled="isSaving" @click="emit('back-to-shelf')">
          ← 返回书架
        </button>
        <div class="book-info">
          <span class="book-title">《{{ bookTitle }}》</span>
          <span class="book-author-badge">{{ bookAuthor }}</span>
          <span class="audit-badge">卷章分割核验</span>
        </div>
      </div>

      <div class="header-center">
        <div class="progress-capsule">
          <span class="capsule-icon">📋</span>
          <span class="capsule-label">已处理:</span>
          <span class="capsule-count">{{ resolvedCount }} / {{ totalAnomaliesCount }}</span>
          <div class="capsule-track">
            <div
              class="capsule-bar"
              :style="{ width: `${totalAnomaliesCount > 0 ? (resolvedCount / totalAnomaliesCount) * 100 : 0}%` }"
            ></div>
          </div>
        </div>
      </div>

      <div class="header-right">
        <button
          v-if="historyStack.length > 0"
          class="audit-undo-btn"
          :disabled="isSaving"
          @click="handleUndo"
          title="撤销上一步操作 (Ctrl+Z)"
        >
          ↩ 撤销
        </button>
        <button class="audit-skip-btn" :disabled="isSaving" @click="skipAndOpenReader">
          不保存并阅读
        </button>
        <button
          class="audit-save-btn"
          :disabled="isSaving"
          @click="saveAndOpenReader"
        >
          {{ isSaving ? "正在落盘..." : "💾 保存修改并进入阅读" }}
        </button>
      </div>
    </header>
    <p v-if="failure" role="alert" class="save-error">{{ failure }}</p>

    <!-- Main Workspace: Left Drawer + Center Continuous Natural Reader View -->
    <div class="audit-workspace" :inert="isSaving">
      <!-- Left Anomaly Navigation Drawer -->
      <aside class="anomaly-drawer">
        <div class="drawer-header">
          <div class="drawer-title-row">
            <span class="drawer-title">核验项目</span>
            <span class="anomaly-total-badge">{{ anomalies.length }} 处</span>
          </div>

          <!-- Filter Pills -->
          <div class="filter-pills">
            <button class="pill-btn" :class="{ active: filterType === 'PENDING' }" @click="filterType = 'PENDING'">
              待处理 ({{ totalAnomaliesCount - resolvedCount }})
            </button>
            <button class="pill-btn" :class="{ active: filterType === 'RESOLVED' }" @click="filterType = 'RESOLVED'">
              已处理 ({{ resolvedCount }})
            </button>
            <button
              class="pill-btn"
              :class="{ active: filterType === 'ALL' }"
              @click="filterType = 'ALL'"
            >
              全部 ({{ anomalies.length }})
            </button>
            <button
              class="pill-btn"
              :class="{ active: filterType === 'SuspiciousShortSection' }"
              @click="filterType = 'SuspiciousShortSection'"
            >
              短正文
            </button>
            <button
              class="pill-btn"
              :class="{ active: filterType === 'SequenceGap' }"
              @click="filterType = 'SequenceGap'"
            >
              跳跃
            </button>
            <button
              class="pill-btn"
              :class="{ active: filterType === 'DuplicateHeading' }"
              @click="filterType = 'DuplicateHeading'"
            >
              重复
            </button>
          </div>
        </div>

        <div class="anomaly-scroll-list">
          <p v-if="filteredAnomalies.length === 0">{{ filterType === 'PENDING' ? '没有待处理项；保存后可进入阅读。' : '没有符合条件的项目。' }}</p>
          <div
            v-for="item in filteredAnomalies"
            :key="item.anomalyId"
            class="anomaly-card"
            :class="{
              active: selectedAnomalyIndex === item.anomalyId,
              resolved: !!resolvedMap[item.anomalyId]
            }"
            @click="selectAnomaly(item.anomalyId, false)"
          >
            <div class="anomaly-card-top">
              <span class="anomaly-idx">#{{ item.anomalyId + 1 }}</span>
              <span
                class="anomaly-tag-badge"
                :class="item.anomaly_type.toLowerCase()"
              >
                {{ formatAnomalyTag(item.anomaly_type) }}
              </span>
              <span
                v-if="resolvedMap[item.anomalyId]"
                class="resolved-status-tag"
              >
                ✓ {{ resolvedMap[item.anomalyId].label }}
              </span>
            </div>

            <div class="anomaly-card-title" :title="item.title">
              {{ item.title }}
            </div>
            <div class="anomaly-card-desc">
              {{ item.description }}
            </div>
          </div>
        </div>
      </aside>

      <!-- Continuous chapter stream with finding markers. -->
      <main class="audit-main-paper">
        <!-- Floating Prompt Banner -->
        <div class="audit-hud-banner">
          <span class="hud-icon">💡</span>
          <span class="hud-text">
            点击标红标题核对正文；原文如此时选择<strong>保留</strong>。选择会移入“已处理”，点击保存后生效。
          </span>
        </div>

        <!-- Scrollable Paper Container with Bidirectional Stitching -->
        <div class="audit-paper-container" ref="readerPaperRef" @scroll="handleAuditScroll">
          <div class="paper-stream-content">
            <div
              v-for="sec in renderedSections"
              :key="sec.id"
              :data-section-id="sec.id"
              class="audit-chapter-stream-block"
            >
              <!-- Chapter Heading: Normal heading or TTS-style red highlighted heading -->
              <h2
                v-if="!isAnomalySection(sec)"
                class="novel-heading-line"
                :class="{ 'is-resolved': isSectionResolved(sec) }"
                @click="openPopoverForSection(sec)"
                title="点击可编辑或调整此章节"
              >
                {{ sec.title }}
              </h2>

              <div
                v-else
                class="novel-suspicious-heading-line"
                @click="openPopoverForSection(sec)"
                title="可疑章节切分点，点击弹出处理框"
              >
                <span class="tts-read-along-red-highlight">
                  {{ sec.title }}
                </span>
                <span class="click-hint-text">（点击处理）</span>
              </div>

              <!-- Original body paragraphs. -->
              <div class="chapter-body-paragraphs">
                <p
                  v-for="(para, pIdx) in getSectionParas(sec)"
                  :key="pIdx"
                  class="audit-para"
                >
                  {{ para }}
                </p>
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>

    <!-- Floating Popover Card (Pops up only when clicking a chapter heading!) -->
    <transition name="popover-fade">
      <div v-if="activePopoverSection" class="audit-floating-popover-backdrop" @click.self="closePopover">
        <div class="audit-popover-card">
          <div class="popover-header">
            <div class="popover-header-title">
              <span class="popover-icon">✦</span>
              <span class="popover-title-text">章节切分处理 · 《{{ activePopoverSection.title }}》</span>
            </div>
            <button class="popover-close-btn" @click="closePopover" title="关闭处理框">✕</button>
          </div>

          <!-- Diagnostic reason -->
          <div v-if="getPopoverAnomaly(activePopoverSection)" class="popover-diagnostic-box">
            <span class="diag-tag">{{ getPopoverAnomaly(activePopoverSection)!.tag }}</span>
            <p class="diag-tip">{{ getPopoverAnomaly(activePopoverSection)!.tip }}</p>
          </div>

          <!-- Inline Title / Volume Edit Row -->
          <div class="popover-editor-row">
            <div class="editor-field title-field">
              <label class="field-label">章节标题</label>
              <input v-model="editTitle" type="text" class="popover-input" placeholder="输入章节标题..." />
            </div>
            <div class="editor-field vol-field">
              <label class="field-label">所属卷</label>
              <input v-model="editVolume" type="text" class="popover-input" placeholder="输入卷名..." />
            </div>
            <button class="popover-btn btn-rename" @click="handleApplyRename" title="保存更名并核验下一处">
              ✏️ 保存更名
            </button>
          </div>

          <!-- Chapter edit and review actions. -->
          <div class="popover-actions-grid">
            <!-- Action 1: Demote into previous chapter -->
            <button
              class="popover-action-btn btn-demote-prev"
              :disabled="getSectionIndex(activePopoverSection) <= 0"
              @click="handleDemoteToPrev"
              title="将本章节取消分割，变回上一章正文段落（适用于对话、引言被误判为章节标题）"
            >
              <span class="btn-icon">↩</span>
              <span class="btn-text">变回正文段落（合并入上一章）</span>
            </button>

            <!-- Action 2: Demote next chapter into this chapter -->
            <button
              class="popover-action-btn btn-demote-next"
              :disabled="getSectionIndex(activePopoverSection) >= sections.length - 1"
              @click="handleDemoteNext"
              title="若紧随其后的下一行（如『3.5小时后……』）被误切成新章，将其取消切分并接续归入当前章"
            >
              <span class="btn-icon">↩</span>
              <span class="btn-text">取消此章切分（变回当前章正文段落）</span>
            </button>

            <!-- Action 3: Delete Section -->
            <button
              class="popover-action-btn btn-delete"
              @click="handleDeleteSection"
              title="彻底删除此虚假章节"
            >
              <span class="btn-icon">🗑️</span>
              <span class="btn-text">删除此章节</span>
            </button>

            <!-- Action 4: Accept as valid -->
            <button
              v-if="!retained(activePopoverSection)"
              class="popover-action-btn btn-accept"
              @click="retain"
              title="保留原有标题和正文，并查看下一处；点击保存后生效"
            >
              <span class="btn-icon">👁️</span>
              <span class="btn-text">保留</span>
            </button>
            <button v-else class="popover-action-btn btn-accept" @click="restore">撤销保留</button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.save-error {
  margin: 0;
  padding: 10px 24px;
  color: #fecaca;
  background: #450a0a;
}
.audit-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #0b1120;
  color: #f1f5f9;
  user-select: none;
  overflow: hidden;
}

/* Header */
.audit-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 24px;
  background: #131c2e;
  border-bottom: 1px solid #1e293b;
  flex-shrink: 0;
  gap: 16px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.back-shelf-btn {
  background: #1e293b;
  border: 1px solid #334155;
  color: #cbd5e1;
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}

.back-shelf-btn:hover {
  background: #334155;
  color: #38bdf8;
}

.book-info {
  display: flex;
  align-items: center;
  gap: 10px;
  overflow: hidden;
}

.book-title {
  font-size: 16px;
  font-weight: 700;
  color: #f8fafc;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.audit-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: rgba(239, 68, 68, 0.2);
  color: #fca5a5;
  border: 1px solid rgba(239, 68, 68, 0.4);
  white-space: nowrap;
}

.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  max-width: 380px;
}

.progress-capsule {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #1e293b;
  border: 1px solid #334155;
  padding: 5px 14px;
  border-radius: 20px;
  font-size: 12px;
  width: 100%;
}

.capsule-icon {
  font-size: 14px;
}

.capsule-label {
  color: #94a3b8;
}

.capsule-count {
  font-weight: 700;
  color: #38bdf8;
  font-family: monospace;
}

.capsule-track {
  flex: 1;
  height: 6px;
  background: #0f172a;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid #334155;
}

.capsule-bar {
  height: 100%;
  background: linear-gradient(90deg, #38bdf8, #10b981);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.audit-undo-btn {
  background: #1e293b;
  border: 1px solid #475569;
  color: #cbd5e1;
  padding: 7px 14px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.audit-undo-btn:hover {
  background: #334155;
  color: #f8fafc;
}

.audit-skip-btn {
  background: transparent;
  border: 1px solid #334155;
  color: #94a3b8;
  padding: 7px 14px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.audit-skip-btn:hover {
  color: #f1f5f9;
  border-color: #475569;
}

.audit-save-btn {
  background: #0284c7;
  border: none;
  color: white;
  padding: 7px 18px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 12px rgba(2, 132, 199, 0.3);
}

.audit-save-btn:hover:not(:disabled) {
  background: #0369a1;
  transform: translateY(-1px);
}

.audit-save-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Workspace */
.audit-workspace {
  display: flex;
  flex: 1;
  overflow: hidden;
}

/* Left Anomaly Navigation Drawer */
.anomaly-drawer {
  width: 320px;
  background: #111827;
  border-right: 1px solid #1e293b;
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.drawer-header {
  padding: 14px 16px;
  border-bottom: 1px solid #1e293b;
  background: #131c2e;
}

.drawer-title-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.drawer-title {
  font-size: 14px;
  font-weight: 700;
  color: #f1f5f9;
}

.anomaly-total-badge {
  font-size: 11px;
  background: rgba(239, 68, 68, 0.2);
  color: #fca5a5;
  padding: 1px 7px;
  border-radius: 10px;
  font-weight: 700;
}

.filter-pills {
  display: flex;
  gap: 6px;
  overflow-x: auto;
}

.pill-btn {
  background: #1e293b;
  border: 1px solid #334155;
  color: #94a3b8;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;
}

.pill-btn.active {
  background: #0284c7;
  color: white;
  border-color: #38bdf8;
  font-weight: 600;
}

.anomaly-scroll-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.anomaly-card {
  padding: 10px 12px;
  background: #182234;
  border: 1px solid #283548;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
}

.anomaly-card:hover {
  background: #1e293b;
  border-color: #38bdf8;
  transform: translateX(2px);
}

.anomaly-card.active {
  background: #1e2d44;
  border-color: #38bdf8;
  box-shadow: 0 0 10px rgba(56, 189, 248, 0.2);
}

.anomaly-card.resolved {
  opacity: 0.65;
  border-color: rgba(16, 185, 129, 0.4);
}

.anomaly-card-top {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.anomaly-idx {
  font-size: 11px;
  font-family: monospace;
  color: #94a3b8;
  font-weight: 700;
}

.anomaly-tag-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 3px;
  font-weight: 700;
  background: rgba(239, 68, 68, 0.2);
  color: #fca5a5;
}

.anomaly-tag-badge.suspiciousshortsection {
  background: rgba(239, 68, 68, 0.2);
  color: #fca5a5;
}

.anomaly-tag-badge.sequencegap {
  background: rgba(56, 189, 248, 0.2);
  color: #7dd3fc;
}

.anomaly-tag-badge.duplicateheading {
  background: rgba(168, 85, 247, 0.2);
  color: #d8b4fe;
}

.resolved-status-tag {
  font-size: 10px;
  color: #34d399;
  font-weight: 600;
  margin-left: auto;
  white-space: nowrap;
}

.anomaly-card-title {
  font-size: 13px;
  font-weight: 600;
  color: #f8fafc;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-bottom: 2px;
}

.anomaly-card-desc {
  font-size: 11px;
  color: #94a3b8;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* Center Continuous Original Article Stream */
.audit-main-paper {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #090e1a;
  overflow: hidden;
}

.audit-hud-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 24px;
  background: #111b2b;
  border-bottom: 1px solid #1e293b;
  font-size: 12px;
  color: #cbd5e1;
  flex-shrink: 0;
}

.hud-icon {
  font-size: 16px;
  color: #eab308;
}

.hud-text strong {
  color: #38bdf8;
}

.audit-paper-container {
  flex: 1;
  overflow-y: auto;
  display: flex;
  justify-content: center;
  padding: 30px 24px 80px;
  user-select: text;
}

.paper-stream-content {
  width: 100%;
  max-width: 840px;
  background: #101726;
  border: 1px solid #1f293d;
  border-radius: 12px;
  padding: 48px 56px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.audit-chapter-stream-block {
  display: flex;
  flex-direction: column;
}

/* Normal Chapter Heading in Prose Flow */
.novel-heading-line {
  scroll-margin-top: 24px;
  font-size: 21px;
  font-weight: 700;
  color: #f1f5f9;
  margin: 32px 0 16px;
  cursor: pointer;
  user-select: text;
  transition: color 0.15s;
}

.novel-heading-line:hover {
  color: #38bdf8;
}

.novel-heading-line.is-resolved {
  color: #10b981;
}

/* Suspicious Chapter Heading: Text highlighted in red like audio TTS read-along! */
.novel-suspicious-heading-line {
  scroll-margin-top: 24px;
  margin: 28px 0 16px;
  display: flex;
  align-items: baseline;
  gap: 10px;
  cursor: pointer;
}

.tts-read-along-red-highlight {
  font-size: 21px;
  font-weight: 700;
  color: #ef4444;
  background: rgba(239, 68, 68, 0.12);
  border-bottom: 2px solid #ef4444;
  border-radius: 4px;
  padding: 2px 10px;
  box-shadow: 0 0 16px rgba(239, 68, 68, 0.2);
  transition: all 0.2s;
  user-select: text;
}

.novel-suspicious-heading-line:hover .tts-read-along-red-highlight {
  background: rgba(239, 68, 68, 0.22);
  box-shadow: 0 0 22px rgba(239, 68, 68, 0.35);
}

.click-hint-text {
  font-size: 12px;
  color: #f87171;
  opacity: 0.75;
  user-select: none;
}

/* Original paragraphs. */
.chapter-body-paragraphs {
  display: flex;
  flex-direction: column;
}

.audit-para {
  text-indent: 2em;
  font-size: 17px;
  line-height: 1.85;
  color: #e2e8f0;
  margin-bottom: 14px;
  text-align: justify;
}

/* Chapter actions over the reading context. */
.audit-floating-popover-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.25);
  backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.audit-popover-card {
  background: #1a2436;
  border: 1px solid #38bdf8;
  border-radius: 14px;
  padding: 24px 28px;
  width: 580px;
  max-width: 92vw;
  box-shadow: 0 25px 60px rgba(0, 0, 0, 0.8), 0 0 35px rgba(56, 189, 248, 0.2);
  display: flex;
  flex-direction: column;
  gap: 16px;
  animation: popoverZoom 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes popoverZoom {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.popover-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #2a374d;
  padding-bottom: 12px;
}

.popover-header-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  font-weight: 700;
  color: #f8fafc;
}

.popover-icon {
  color: #f87171;
  font-size: 18px;
}

.popover-close-btn {
  background: transparent;
  border: none;
  color: #94a3b8;
  font-size: 18px;
  cursor: pointer;
  padding: 4px;
  line-height: 1;
}

.popover-close-btn:hover {
  color: #ffffff;
}

.popover-diagnostic-box {
  background: #0f172a;
  border: 1px solid #283548;
  border-radius: 8px;
  padding: 10px 14px;
}

.diag-tag {
  display: inline-block;
  font-size: 11px;
  font-weight: 700;
  color: #fca5a5;
  background: rgba(239, 68, 68, 0.2);
  padding: 2px 6px;
  border-radius: 4px;
  margin-bottom: 4px;
}

.diag-tip {
  margin: 0;
  font-size: 12px;
  color: #cbd5e1;
  line-height: 1.5;
}

.popover-editor-row {
  display: flex;
  gap: 10px;
  align-items: flex-end;
}

.editor-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
}

.editor-field.vol-field {
  flex: 0.6;
}

.field-label {
  font-size: 11px;
  color: #94a3b8;
  font-weight: 600;
}

.popover-input {
  background: #0f172a;
  border: 1px solid #334155;
  border-radius: 6px;
  padding: 7px 10px;
  font-size: 13px;
  color: #f8fafc;
  outline: none;
}

.popover-input:focus {
  border-color: #38bdf8;
}

.popover-btn.btn-rename {
  background: #334155;
  border: none;
  color: #f1f5f9;
  padding: 7px 14px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  height: 34px;
  transition: all 0.15s;
}

.popover-btn.btn-rename:hover {
  background: #475569;
}

.popover-actions-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 4px;
}

.popover-action-btn {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: 8px;
  border: none;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.15s;
  text-align: left;
}

.popover-action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.popover-action-btn.btn-demote-prev {
  background: rgba(16, 185, 129, 0.15);
  border: 1px solid #10b981;
  color: #6ee7b7;
}

.popover-action-btn.btn-demote-prev:hover:not(:disabled) {
  background: #059669;
  color: white;
}

.popover-action-btn.btn-demote-next {
  background: rgba(56, 189, 248, 0.15);
  border: 1px solid #0284c7;
  color: #7dd3fc;
}

.popover-action-btn.btn-demote-next:hover:not(:disabled) {
  background: #0284c7;
  color: white;
}

.popover-action-btn.btn-delete {
  background: rgba(239, 68, 68, 0.12);
  border: 1px solid rgba(239, 68, 68, 0.4);
  color: #fca5a5;
}

.popover-action-btn.btn-delete:hover:not(:disabled) {
  background: #dc2626;
  color: white;
}

.popover-action-btn.btn-accept {
  background: #253347;
  border: 1px solid #3d4f6a;
  color: #e2e8f0;
}

.popover-action-btn.btn-accept:hover:not(:disabled) {
  background: #33445e;
}

.btn-icon {
  font-size: 15px;
}

/* Transitions */
.popover-fade-enter-active,
.popover-fade-leave-active {
  transition: opacity 0.2s ease;
}

.popover-fade-enter-from,
.popover-fade-leave-to {
  opacity: 0;
}
</style>
