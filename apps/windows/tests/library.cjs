/** Counterexamples for persistence failures, stale loads, and displayed sentence positions. */
const assert = require("node:assert/strict");
const { test } = require("node:test");
const loader = require("./support/loader.cjs");

/** Produce a complete book with one body-only section. */
function novel(title = "Original") {
  return {
    book: { id: "book", title, author: "Author", description: "", chapters: 1 },
    sections: [{ id: "book:0", index: 0, title, volume: "正文", body: "甲。乙！”\n\n丙？", chunks: [] }]
  };
}

test("body-only paragraphs and sentence navigation use identical zero-based positions", () => {
  const text = loader()("text.ts");
  const section = novel().sections[0];
  section.body = "甲。乙！”\n\n丙？";
  const groups = text.paragraphs(section);
  assert.deepEqual(Array.from(groups.flatMap(group => group.chunks.map(chunk => chunk.index))), [0, 1, 2]);
  assert.deepEqual(Array.from(text.contents(section)), ["甲。", "乙！”", "丙？"]);
  assert.deepEqual(Array.from(text.paragraphs({ ...section, body: " \r\n\t " })), []);
  assert.equal(section.chunks.length, 0);
});

test("failed native deletion preserves catalog and selected book", async () => {
  const store = loader({ invoke: async () => { throw new Error("disk locked"); } })("stores/library.ts");
  store.importNovel(novel());
  await assert.rejects(store.deleteBook("book"), /disk locked/);
  assert.equal(store.books.value.length, 1);
  assert.equal(store.activeBookId.value, "book");
});

test("concurrent hydration is coalesced and cannot overwrite a subsequent reimport", async () => {
  let complete;
  let calls = 0;
  const store = loader({ invoke: async () => { calls++; return new Promise(resolve => { complete = resolve; }); } })("stores/library.ts");
  store.books.value = [{ ...novel().book, currentChapterIndex: 0 }];
  const first = store.ensureBookLoaded("book");
  const second = store.ensureBookLoaded("book");
  assert.equal(calls, 1);
  store.importNovel(novel("Replacement"));
  complete(novel());
  await Promise.all([first, second]);
  assert.equal(store.books.value[0].sections[0].title, "Replacement");
});

test("deleted books stay absent after late hydration", async () => {
  let complete;
  const store = loader({ invoke: async command => command === "load_novel" ? new Promise(resolve => { complete = resolve; }) : undefined })("stores/library.ts");
  store.books.value = [{ ...novel().book, currentChapterIndex: 0 }];
  const pending = store.ensureBookLoaded("book");
  await store.deleteBook("book");
  complete(novel());
  await pending;
  assert.equal(store.books.value.length, 0);
});

test("empty-section saves clear current text and stop playback", async () => {
  const store = loader({ invoke: async () => ({ anomalies: [] }) })("stores/library.ts");
  store.importNovel(novel());
  store.activeBook.value.audio = [new Blob(["audio"], { type: "audio/mpeg" })];
  store.play();
  try {
    assert.equal(store.isPlaying.value, true);
    await store.saveAuditedSections("book", [], []);
    assert.equal(store.books.value[0].currentChapter, "");
    assert.equal(store.books.value[0].sentences.length, 0);
    assert.equal(store.isPlaying.value, false);
  } finally { store.pause(); }
});

test("load errors remain observable and a later explicit request can retry", async () => {
  let fail = true;
  const store = loader({ invoke: async () => { if (fail) throw new Error("invalid JSON"); return novel(); } })("stores/library.ts");
  store.books.value = [{ ...novel().book, currentChapterIndex: 0 }];
  await assert.rejects(store.ensureBookLoaded("book"), /invalid JSON/);
  assert.match(store.failure.value, /invalid JSON/);
  fail = false;
  await store.ensureBookLoaded("book");
  assert.equal(store.failure.value, "");
  assert.equal(store.books.value[0].sections.length, 1);
});

test("browser preview retains the entire text without calling native persistence", () => {
  const store = loader({ isTauri: () => false, invoke: () => assert.fail("no native IPC") })("stores/library.ts");
  const body = "一句。".repeat(40);
  store.importBook("Preview", "Author", "TXT", body);
  assert.equal(store.books.value[0].sections[0].body, body);
  assert.equal(store.books.value[0].sentences.length, 40);
});

test("text-only books cannot start narration regardless of their filename", () => {
  const store = loader()("stores/library.ts");
  store.importNovel(novel("Text-only.saga"));
  assert.equal(store.hasAudio.value, false);
  store.play();
  assert.equal(store.isPlaying.value, false);
  store.togglePlay();
  assert.equal(store.isPlaying.value, false);
});

test("audio availability follows nonempty loaded audio resources of the selected book", () => {
  const store = loader()("stores/library.ts");
  store.importNovel(novel());
  const book = store.activeBook.value;
  book.audio = [new Blob([], { type: "audio/mpeg" }), new Blob(["text"], { type: "text/plain" })];
  assert.equal(store.hasAudio.value, false);
  book.audio.push(new Blob([new Uint8Array([0xff, 0xfb])], { type: "audio/mpeg" }));
  assert.equal(store.hasAudio.value, true);
  try {
    store.play();
    assert.equal(store.isPlaying.value, true);
    book.audio = [];
    assert.equal(store.hasAudio.value, false);
    assert.equal(store.isPlaying.value, false);
    book.audio = [new Blob(["audio"], { type: "audio/mpeg" })];
    assert.equal(store.hasAudio.value, true);
    store.importNovel({ ...novel("Other"), book: { ...novel().book, id: "other" } });
    assert.equal(store.hasAudio.value, false);
    store.selectBook("book");
    assert.equal(store.hasAudio.value, true);
    store.importNovel(novel("Replacement"));
    assert.equal(store.hasAudio.value, false);
  } finally { store.pause(); }
});

test("selecting chapters updates reading progress percentage and lastRead status", async () => {
  const store = loader()("stores/library.ts");
  const total = 3167;
  const sections = Array.from({ length: total }, (_, index) => ({
    id: `book:${index}`,
    index,
    title: `第${index + 1}章`,
    volume: "正文",
    body: "测试正文。",
    chunks: []
  }));
  store.importNovel({
    book: { id: "book", title: "修真聊天群", author: "圣骑士的传说", description: "", chapters: total },
    sections
  });
  const book = store.activeBook.value;
  assert.equal(book.progress, 0);

  await store.setBookChapter("book", 300);
  assert.equal(book.currentChapterIndex, 300);
  assert.equal(book.currentChapter, "第301章");
  assert.equal(book.progress, Math.round((301 / total) * 100));
  assert.equal(book.lastRead, "最近阅读");

  await store.setBookChapter("book", total - 1);
  assert.equal(book.currentChapterIndex, total - 1);
  assert.equal(book.progress, 100);
});

test("loadStoredNovels restores saved chapter, paragraph, and progress percentage", async () => {
  const summaries = [{
    id: "sample-book",
    title: "示例书名",
    author: "作者",
    description: "简介",
    chapters: 100,
    format: "SAGA",
    first_chapter: "第一章",
    last_chapter_index: 49,
    last_paragraph_index: 8,
    last_scroll_ratio: 0.35
  }];
  const store = loader({ invoke: async (cmd) => cmd === "load_book_summaries" ? summaries : undefined })("stores/library.ts");
  await store.loadStoredNovels();

  assert.equal(store.books.value.length, 1);
  const book = store.books.value[0];
  assert.equal(book.currentChapterIndex, 49);
  assert.equal(book.currentParagraphIndex, 8);
  assert.equal(book.currentScrollRatio, 0.35);
  assert.equal(book.progress, 50);
  assert.equal(book.lastRead, "最近阅读");
});
