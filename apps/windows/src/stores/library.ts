/** Session library state; publish persistent mutations only after native success. */
import { ref, computed, markRaw, watch } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { contents, paragraphs } from "../text";
import type { Book } from "../../../../protocol/bindings/Book";
import type { BookSummary } from "../../../../protocol/bindings/BookSummary";
import type { Novel } from "../../../../protocol/bindings/Novel";
import type { Section } from "../../../../protocol/bindings/Section";
import type { NovelValidation } from "../../../../protocol/bindings/NovelValidation";

/** Metadata and transient reading state; absent sections mean not yet loaded. */
export interface BookItem extends Book {
  /** Stored format label; TXT or native SAGA package. */
  format: "TXT" | "SAGA";
  /** Reading progress percentage (0 - 100). */
  progress: number;
  /** Session provenance label or last read status. */
  lastRead: string;
  /** Decorative cover background. */
  coverGradient: string;
  /** Current section title, empty for a book without sections. */
  currentChapter: string;
  /** Zero-based section position. */
  currentChapterIndex: number;
  /** Zero-based paragraph position within chapter. */
  currentParagraphIndex?: number;
  /** Scroll ratio within chapter (0.0 to 1.0). */
  currentScrollRatio?: number;
  /** Loaded source graph; undefined until hydration completes. */
  sections?: Section[];
  /** Loaded audio bytes; current TXT imports have none. Filenames do not establish availability. */
  audio?: Blob[];
  /** Current sentences used by the visual playback demonstration. */
  sentences: string[];
}

// Shared catalog, selection, visual playback state, and last observable load failure.
export const books = ref<BookItem[]>([]);
export const activeBookId = ref("");
export const activeSentenceIndex = ref(0);
export const isPlaying = ref(false);
export const speechRate = ref(1.0);
export const failure = ref("");
export const activeAuditReport = ref<NovelValidation | null>(null);
export const activeAuditNovel = ref<Novel | null>(null);
export const isGlobalBusy = ref(false);

const STORED_BANNER_KEY = "saga_shelf_show_hero_banner";
export const showHeroBanner = ref<boolean>(
  typeof localStorage !== "undefined"
    ? localStorage.getItem(STORED_BANNER_KEY) !== "false"
    : true
);

watch(showHeroBanner, (val) => {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORED_BANNER_KEY, String(val));
  }
});

/** Selected catalog entry, with an initial first-book fallback. */
export const activeBook = computed<BookItem | null>(() => books.value.find((book) => book.id === activeBookId.value) || books.value[0] || null);
/** Only nonempty, loaded audio resources enable narration controls for this book. */
export const hasAudio = computed(() => activeBook.value?.audio?.some(resource =>
  resource instanceof Blob && resource.size > 0 && resource.type.startsWith("audio/")) ?? false);
/** Selected source section; no section is distinct from an empty body. */
export const activeSection = computed<Section | null>(() => {
  const book = activeBook.value;
  return book?.sections?.[book.currentChapterIndex] || null;
});
/** Render the same paragraph projection used by continuous reading. */
export const activeParagraphs = computed(() => activeSection.value ? paragraphs(activeSection.value) : []);

// Object identity prevents late hydration from replacing a reimported book.
const loading = new Map<BookItem, Promise<void>>();
// Successful mutations invalidate earlier hydration and catalog responses for the same identity.
const versions = new Map<string, number>();
const deleting = new Set<string>();
let revision = 0;
let playTimer: ReturnType<typeof setInterval> | null = null;

/** Record one successfully published catalog mutation. */
function revise(id: string): void {
  versions.set(id, ++revision);
}

// Resource removal must also stop an already running session timer.
watch(hasAudio, available => { if (!available) pause(); }, { flush: "sync" });

/** Select a book and observe hydration failures through shared status. */
export function selectBook(id: string): void {
  pause();
  activeBookId.value = id;
  activeSentenceIndex.value = 0;
  void ensureBookLoaded(id).catch(() => { });
}

/** Start a visual sentence timer; this does not synthesize or play audio. */
export function play(): void {
  if (!hasAudio.value || isPlaying.value || !activeBook.value?.sentences.length) return;
  isPlaying.value = true;
  const rate = Number.isFinite(speechRate.value) && speechRate.value > 0 ? speechRate.value : 1;
  playTimer = setInterval(() => {
    const length = activeBook.value?.sentences.length || 0;
    if (!hasAudio.value || length === 0) pause();
    else activeSentenceIndex.value = (activeSentenceIndex.value + 1) % length;
  }, Math.max(800, Math.floor(2200 / rate)));
}

/** Release the session's single playback timer. */
export function pause(): void {
  isPlaying.value = false;
  if (playTimer !== null) clearInterval(playTimer);
  playTimer = null;
}

/** Toggle the visual playback demonstration for the selected book. */
export function togglePlay(): void {
  if (isPlaying.value) pause();
  else play();
}

/** Advance within the current section without wrapping. */
export function nextSentence(): void {
  const length = activeBook.value?.sentences.length || 0;
  if (length > 0) activeSentenceIndex.value = Math.min(activeSentenceIndex.value + 1, length - 1);
}

/** Move backwards within the current section without wrapping. */
export function prevSentence(): void {
  activeSentenceIndex.value = Math.max(0, activeSentenceIndex.value - 1);
}

/** Keep the existing chapter-based shelf percentage in one place. */
function percentage(index: number, total: number): number {
  if (total <= 1 || index <= 0) return 0;
  if (index >= total - 1) return 100;
  return Math.min(99, Math.max(1, Math.round(((index + 1) / total) * 100)));
}

/** Refresh derived text and clamp a saved anchor after hydration or a committed edit. */
function project(book: BookItem, index: number): void {
  const sections = book.sections || [];
  const previous = book.currentChapterIndex;
  book.currentChapterIndex = Math.max(0, Math.min(Number.isInteger(index) ? index : 0, sections.length - 1));
  const section = sections[book.currentChapterIndex];
  book.currentChapter = section?.title || "";
  book.sentences = section ? contents(section) : [];
  const total = sections.length || book.chapters || 0;
  book.progress = percentage(book.currentChapterIndex, total);
  const count = section ? paragraphs(section).length : 0;
  const paragraph = book.currentParagraphIndex ?? 0;
  const position = previous === book.currentChapterIndex && Number.isInteger(paragraph)
    ? Math.max(0, Math.min(paragraph, count - 1)) : 0;
  const ratio = book.currentScrollRatio ?? 0;
  book.currentParagraphIndex = position;
  book.currentScrollRatio = previous === book.currentChapterIndex && position === paragraph && Number.isFinite(ratio)
    ? Math.max(0, Math.min(1, ratio)) : 0;
  if (book.currentChapterIndex > 0 || position > 0) {
    book.lastRead = "最近阅读";
  }
  if (activeBookId.value === book.id) {
    pause();
    activeSentenceIndex.value = 0;
  }
}

/** Hydrate if necessary, then select a valid section; load errors remain visible. */
export async function setBookChapter(bookId: string, chapterIndex: number): Promise<void> {
  const current = books.value.find((item) => item.id === bookId);
  if (current?.sections) {
    if (current.sections[chapterIndex]) project(current, chapterIndex);
    return;
  }
  await ensureBookLoaded(bookId);
  const book = books.value.find((item) => item.id === bookId);
  if (book?.sections?.[chapterIndex]) project(book, chapterIndex);
}

/** Select the next chapter when present. */
export function nextChapter(): void {
  const book = activeBook.value;
  if (book) void setBookChapter(book.id, book.currentChapterIndex + 1).catch(() => { });
}

/** Select the previous chapter when present. */
export function prevChapter(): void {
  const book = activeBook.value;
  if (book) void setBookChapter(book.id, book.currentChapterIndex - 1).catch(() => { });
}

/** Publish a persisted native import or an explicitly transient preview. */
export function importNovel(novel: Novel, format: "TXT" | "SAGA" = "TXT"): BookItem {
  pause();
  const previous = books.value.find(item => item.id === novel.book.id);
  const book: BookItem = {
    ...novel.book, format, progress: 0, lastRead: isTauri() ? "本地导入" : "浏览器临时预览",
    coverGradient: "linear-gradient(135deg, #1e3a8a 0%, #312e81 100%)",
    currentChapter: "", currentChapterIndex: previous?.currentChapterIndex ?? 0,
    currentParagraphIndex: previous?.currentParagraphIndex ?? 0,
    currentScrollRatio: previous?.currentScrollRatio ?? 0,
    sections: markRaw(novel.sections), sentences: []
  };
  project(book, book.currentChapterIndex);
  revise(book.id);
  books.value = [book, ...books.value.filter((item) => item.id !== book.id)];
  activeBookId.value = book.id;
  activeSentenceIndex.value = 0;
  if (pendingProgress.has(book.id) || writing?.bookId === book.id) {
    pendingProgress.set(book.id, {
      bookId: book.id, chapterIndex: book.currentChapterIndex,
      paragraphIndex: book.currentParagraphIndex ?? 0, scrollRatio: book.currentScrollRatio ?? 0
    });
  }
  return book;
}

/** Preview the entire browser TXT as one section, without native parsing or persistence. */
export function importBook(title: string, author: string, format: "TXT" | "SAGA" = "TXT", body = ""): void {
  if (format !== "TXT") throw new Error("浏览器临时预览仅支持 TXT；请在桌面端导入 .saga 文件");
  const id = crypto.randomUUID();
  importNovel({
    book: { id, title, author: author || "未知作者", description: "", chapters: 1 },
    sections: [{ id: id + ":0", index: 0, title: "正文", volume: "正文", body, chunks: [] }]
  }, format);
}

/** Delete storage before removing catalog state; rejection preserves the shelf. */
export async function deleteBook(id: string): Promise<void> {
  if (deleting.has(id)) return;
  deleting.add(id);
  try {
    // Finish an already issued write before deletion; retain queued progress if deletion is rejected.
    if (progressWrite) await progressWrite.catch(() => { });
    if (isTauri()) await invoke("delete_novel", { id });
    pendingProgress.delete(id);
    revise(id);
    books.value = books.value.filter((book) => book.id !== id);
    if (activeBookId.value === id) {
      pause();
      activeBookId.value = books.value[0]?.id || "";
      activeSentenceIndex.value = 0;
    }
    if (activeAuditNovel.value?.book.id === id) {
      activeAuditNovel.value = null;
      activeAuditReport.value = null;
    }
  } finally {
    deleting.delete(id);
  }
}

/** Coalesce loads by catalog object, rejecting failures and ignoring stale responses. */
export async function ensureBookLoaded(bookId: string): Promise<void> {
  const target = books.value.find((book) => book.id === bookId);
  if (!target || target.sections !== undefined || !isTauri()) return;
  const pending = loading.get(target);
  if (pending) return pending;
  const version = versions.get(bookId);
  const operation = (async () => {
    try {
      const novel = await invoke<Novel | null>("load_novel", { id: bookId });
      if (!books.value.includes(target) || versions.get(bookId) !== version || target.sections !== undefined) return;
      if (!novel) throw new Error("书籍已不存在，请重新导入");
      target.sections = markRaw(novel.sections);
      target.chapters = novel.sections.length;
      project(target, target.currentChapterIndex);
      if (failure.value.startsWith("加载书籍失败：")) failure.value = "";
    } catch (error) {
      if (!books.value.includes(target) || versions.get(bookId) !== version) return;
      failure.value = "加载书籍失败：" + error;
      throw error;
    } finally {
      loading.delete(target);
    }
  })();
  loading.set(target, operation);
  return operation;
}

/** Publish audited sections after native text and retention decisions commit. */
export async function saveAuditedSections(bookId: string, sections: Section[], retained: string[]): Promise<NovelValidation> {
  const target = books.value.find((book) => book.id === bookId);
  const version = versions.get(bookId);
  const snapshot = sections.map(section => ({ ...section, chunks: section.chunks.map(chunk => ({ ...chunk })) }));
  if (progressWrite || pendingProgress.size > 0) await flushReadingProgress();
  const report = await invoke<NovelValidation>("update_novel_sections", { bookId, sections: snapshot, retained: [...retained] });
  if (target && books.value.includes(target) && versions.get(bookId) === version) {
    revise(bookId);
    target.sections = markRaw(snapshot);
    target.chapters = snapshot.length;
    project(target, target.currentChapterIndex);
  }
  return report;
}

/** Merge persisted summaries; full bodies load only when selected. */
export async function loadStoredNovels(): Promise<void> {
  if (!isTauri()) return;
  const started = revision;
  try {
    const summaries = await invoke<BookSummary[]>("load_book_summaries");
    for (const summary of summaries) {
      if ((versions.get(summary.id) ?? 0) > started || books.value.some((book) => book.id === summary.id)) continue;
      const total = summary.chapters || 0;
      const curIdx = Math.max(0, Math.min(summary.last_chapter_index || 0, total - 1));
      books.value.push({
        id: summary.id,
        title: summary.title,
        author: summary.author,
        description: summary.description,
        chapters: summary.chapters,
        format: summary.format === "SAGA" ? "SAGA" : "TXT",
        progress: percentage(curIdx, total),
        lastRead: curIdx > 0 || (summary.last_paragraph_index || 0) > 0 ? "最近阅读" : "本地数据库",
        coverGradient: "linear-gradient(135deg, #1e3a8a 0%, #312e81 100%)",
        currentChapter: summary.first_chapter || "正文",
        currentChapterIndex: curIdx,
        currentParagraphIndex: summary.last_paragraph_index || 0,
        currentScrollRatio: summary.last_scroll_ratio || 0,
        sentences: []
      });
    }
    if (!activeBookId.value) activeBookId.value = books.value[0]?.id || "";
    if (failure.value.startsWith("读取书架失败：")) failure.value = "";

    const recent = books.value[0];
    if (recent && recent.sections === undefined) {
      if (typeof setTimeout !== "undefined") {
        setTimeout(() => { void ensureBookLoaded(recent.id).catch(() => { }); }, 50);
      }
    }
  } catch (error) {
    failure.value = "读取书架失败：" + error;
    throw error;
  }
}

let progressSaveTimer: ReturnType<typeof setTimeout> | null = null;
/** Latest unsaved anchor for one book; ratios are finite and within [0, 1]. */
interface Position {
  bookId: string;
  chapterIndex: number;
  paragraphIndex: number;
  scrollRatio: number;
}
const pendingProgress = new Map<string, Position>();
let progressWrite: Promise<void> | null = null;
let writing: Position | null = null;

/** Drain queued positions in order, sharing the active write and retaining failed positions for retry. */
export async function flushReadingProgress(): Promise<void> {
  if (progressSaveTimer !== null) {
    clearTimeout(progressSaveTimer);
    progressSaveTimer = null;
  }
  if (!isTauri()) return;
  if (progressWrite) {
    await progressWrite;
    if (pendingProgress.size > 0) await flushReadingProgress();
    return;
  }
  if (pendingProgress.size === 0) return;
  const operation = (async () => {
    while (pendingProgress.size > 0) {
      const payload = [...pendingProgress.values()].find(position => !deleting.has(position.bookId));
      if (!payload) break;
      pendingProgress.delete(payload.bookId);
      writing = payload;
      try {
        await invoke("update_reading_progress", { ...payload });
        if (failure.value.startsWith("保存阅读进度失败：")) failure.value = "";
      } catch (error) {
        if (books.value.some(book => book.id === payload.bookId) && !pendingProgress.has(payload.bookId)) {
          pendingProgress.set(payload.bookId, payload);
        }
        failure.value = "保存阅读进度失败：" + error + "；位置仍保留在当前会话，可再次尝试保存";
        throw error;
      } finally {
        writing = null;
      }
    }
  })();
  progressWrite = operation;
  try { await operation; }
  finally {
    if (progressWrite === operation) progressWrite = null;
  }
}

/** Updates in-memory position and debounces persistent write to SQLite. */
export function saveReadingProgress(
  bookId: string,
  chapterIndex: number,
  paragraphIndex: number,
  scrollRatio: number
): void {
  const target = books.value.find((b) => b.id === bookId);
  const total = target?.sections?.length ?? target?.chapters ?? 0;
  if (!target || deleting.has(bookId) || !Number.isInteger(chapterIndex) || chapterIndex < 0 || chapterIndex >= Math.max(1, total)
    || !Number.isInteger(paragraphIndex) || paragraphIndex < 0 || !Number.isFinite(scrollRatio) || scrollRatio < 0 || scrollRatio > 1) return;
  if (target.currentChapterIndex !== chapterIndex) project(target, chapterIndex);
  target.currentParagraphIndex = paragraphIndex;
  target.currentScrollRatio = scrollRatio;
  target.progress = percentage(chapterIndex, total);
  if (chapterIndex > 0 || paragraphIndex > 0) {
    target.lastRead = "最近阅读";
  }
  const sec = target.sections?.[chapterIndex];
  if (sec) {
    target.currentChapter = sec.title;
  }
  if (!isTauri()) return;
  pendingProgress.set(bookId, { bookId, chapterIndex, paragraphIndex, scrollRatio });
  if (typeof setTimeout !== "undefined") {
    if (progressSaveTimer !== null) {
      clearTimeout(progressSaveTimer);
    }
    progressSaveTimer = setTimeout(() => {
      void flushReadingProgress().catch(() => { });
    }, 300);
  } else {
    void flushReadingProgress().catch(() => { });
  }
}
