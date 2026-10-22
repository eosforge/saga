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
  /** Reserved percentage; durable reading progress is not implemented. */
  progress: number;
  /** Session provenance label, not a durable read timestamp. */
  lastRead: string;
  /** Decorative cover background. */
  coverGradient: string;
  /** Current section title, empty for a book without sections. */
  currentChapter: string;
  /** Zero-based section position. */
  currentChapterIndex: number;
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
let playTimer: ReturnType<typeof setInterval> | null = null;

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

/** Refresh dependent reading state after hydration or a committed edit. */
function project(book: BookItem, index: number): void {
  const sections = book.sections || [];
  book.currentChapterIndex = Math.max(0, Math.min(index, sections.length - 1));
  const section = sections[book.currentChapterIndex];
  book.currentChapter = section?.title || "";
  book.sentences = section ? contents(section) : [];
  const total = sections.length || book.chapters || 0;
  book.progress = total <= 1 || book.currentChapterIndex <= 0
    ? 0
    : book.currentChapterIndex >= total - 1
      ? 100
      : Math.min(99, Math.max(1, Math.round(((book.currentChapterIndex + 1) / total) * 100)));
  if (book.currentChapterIndex > 0) {
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
  const book: BookItem = {
    ...novel.book, format, progress: 0, lastRead: isTauri() ? "本地导入" : "浏览器临时预览",
    coverGradient: "linear-gradient(135deg, #1e3a8a 0%, #312e81 100%)",
    currentChapter: "", currentChapterIndex: 0, sections: markRaw(novel.sections), sentences: []
  };
  project(book, 0);
  books.value = [book, ...books.value.filter((item) => item.id !== book.id)];
  activeBookId.value = book.id;
  activeSentenceIndex.value = 0;
  return book;
}

/** Preview the entire browser TXT as one section, without native parsing or persistence. */
export function importBook(title: string, author: string, format: "TXT" | "SAGA" = "TXT", body = ""): void {
  const id = crypto.randomUUID();
  importNovel({
    book: { id, title, author: author || "未知作者", description: "", chapters: 1 },
    sections: [{ id: id + ":0", index: 0, title: "正文", volume: "正文", body, chunks: [] }]
  }, format);
}

/** Delete storage before removing catalog state; rejection preserves the shelf. */
export async function deleteBook(id: string): Promise<void> {
  if (isTauri()) await invoke("delete_novel", { id });
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
}

/** Coalesce loads by catalog object, rejecting failures and ignoring stale responses. */
export async function ensureBookLoaded(bookId: string): Promise<void> {
  const target = books.value.find((book) => book.id === bookId);
  if (!target || target.sections !== undefined || !isTauri()) return;
  const pending = loading.get(target);
  if (pending) return pending;
  const operation = (async () => {
    try {
      const novel = await invoke<Novel | null>("load_novel", { id: bookId });
      if (!books.value.includes(target)) return;
      if (!novel) throw new Error("书籍已不存在，请重新导入");
      target.sections = markRaw(novel.sections);
      target.chapters = novel.sections.length;
      project(target, target.currentChapterIndex);
      failure.value = "";
    } catch (error) {
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
  const report = await invoke<NovelValidation>("update_novel_sections", { bookId, sections, retained });
  const target = books.value.find((book) => book.id === bookId);
  if (target) {
    target.sections = markRaw(sections);
    target.chapters = sections.length;
    project(target, target.currentChapterIndex);
  }
  return report;
}

/** Merge persisted summaries; full bodies load only when selected. */
export async function loadStoredNovels(): Promise<void> {
  if (!isTauri()) return;
  try {
    const summaries = await invoke<BookSummary[]>("load_book_summaries");
    for (const summary of summaries) {
      if (books.value.some((book) => book.id === summary.id)) continue;
      books.value.push({
        id: summary.id, title: summary.title, author: summary.author, description: summary.description,
        chapters: summary.chapters, format: summary.format === "SAGA" ? "SAGA" : "TXT", progress: 0,
        lastRead: "本地数据库", coverGradient: "linear-gradient(135deg, #1e3a8a 0%, #312e81 100%)",
        currentChapter: summary.first_chapter || "正文", currentChapterIndex: 0, sentences: []
      });
    }
    if (!activeBookId.value) activeBookId.value = books.value[0]?.id || "";
    failure.value = "";

    const recent = books.value[0];
    if (recent && recent.sections === undefined) {
      setTimeout(() => { void ensureBookLoaded(recent.id).catch(() => { }); }, 50);
    }
  } catch (error) {
    failure.value = "读取书架失败：" + error;
    throw error;
  }
}
