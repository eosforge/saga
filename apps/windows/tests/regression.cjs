/** Isolated behavioral counterexamples; actual Vue handlers and TypeScript modules. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
const root = path.resolve(__dirname, '..');
const ts = require(path.join(root, 'node_modules/typescript'));
const vue = require(path.join(root, 'node_modules/vue'));

function timers() {
  let counter = 0;
  const callbacks = new Map();
  return {
    setTimeout(cb) { callbacks.set(++counter, cb); return counter; },
    clearTimeout(id) { callbacks.delete(id); },
    run() { const list = [...callbacks.values()]; callbacks.clear(); for (const cb of list) cb(); },
    callbacks,
  };
}
function graph(invoke = async () => { }, native = true) {
  const cache = new Map(), clock = timers(), messages = [];
  function load(relative) {
    const filename = path.resolve(root, 'src', relative);
    if (cache.has(filename)) return cache.get(filename);
    const exports = {}; cache.set(filename, exports);
    const code = ts.transpileModule(fs.readFileSync(filename, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
    vm.runInNewContext(code, {
exports, console: { warn: (...args) => messages.push(args), error: (...args) => messages.push(args) }, Blob,
      crypto: require('node:crypto').webcrypto, ...clock,
      setInterval: clock.setTimeout, clearInterval: clock.clearTimeout,
      require(name) {
        if (name === 'vue') return vue;
        if (name === '@tauri-apps/api/core') return { invoke, isTauri: () => native };
        if (name.startsWith('.')) return load(path.resolve(path.dirname(filename), name + '.ts'));
        throw Error(name);
      }
    });
    return exports;
  }
  return { store: load('stores/library.ts'), text: load('text.ts'), clock, messages };
}
function chapter(index, chunks = true) {
  const id = 'book:' + index, body = 'Body ' + index + '.';
  return { id, index, title: 'Chapter ' + index, volume: 'Volume', body, chunks: chunks ? [{ id: id + ':c0', index: 0, paragraph: 0, chapter: id, text: body }] : [] };
}
function novel(count = 3) {
  return { book: { id: 'book', title: 'Fixture', author: 'Author', description: '', chapters: count }, sections: Array.from({ length: count }, (_, i) => chapter(i)) };
}
function component(name, library, exposed, native = true) {
  const source = fs.readFileSync(path.join(root, 'src/views', name + '.vue'), 'utf8').match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1];
  const code = ts.transpileModule(source + '\nexport { ' + exposed + ' };', { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const exports = {}, clock = timers(), events = [], lifecycle = {};
  const document = { querySelector: () => null };
  const window = { addEventListener() { }, removeEventListener() { } };
  vm.runInNewContext(code, {
exports, console: { warn() { }, error() { } }, defineEmits: () => event => events.push(event), ...clock,
    document, window,
    require(dependency) {
      if (dependency === 'vue') return { ...vue, onMounted: cb => { lifecycle.mount = cb; }, onUnmounted: cb => { lifecycle.unmount = cb; }, watch() { } };
      if (dependency === '../stores/library') return library;
      if (dependency === '../text') return graph().text;
      if (dependency === '@tauri-apps/api/core') return { isTauri: () => native, invoke: async () => { } };
      throw Error(dependency);
    }
  });
  return { ...exports, events, lifecycle, clock, document };
}
function audit(sections) {
  let received;
  const library = {
    activeAuditNovel: vue.ref({ book: { id: 'book', title: 'Fixture' }, sections }),
    activeAuditReport: vue.ref({ anomalies: [] }),
    activeBook: vue.ref(null),
    isGlobalBusy: vue.ref(false),
    saveAuditedSections: async (...args) => { received = args; return { anomalies: [] }; },
    selectBook() { },
  };
  const ui = component('Audit', library, 'sections, openPopoverForSection, handleDemoteToPrev, mergeIncludeTitle, saveAndOpenReader, readerPaperRef');
  ui.lifecycle.mount();
  return { ...ui, get received() { return received; } };
}
test('audit: merged body must match text displayed before and after reload', async () => {
  const ui = audit([chapter(0), chapter(1, false)]);
  ui.openPopoverForSection(ui.sections.value[1]);
  ui.mergeIncludeTitle.value = true;
  ui.handleDemoteToPrev();
  const sec = ui.sections.value[0], text = graph().text;
  const fromCache = Array.from(text.contents(sec)).join('');
  const fromBody = Array.from(text.contents({ ...sec, chunks: [] })).join('');
  assert.equal(fromCache, fromBody, 'mixed eager/lazy chunks lose merged text in the current reader');
});
test('audit: repeated merge into one chapter must retain unique sentence IDs', async () => {
  const ui = audit([chapter(0), chapter(1), chapter(2)]);
  ui.openPopoverForSection(ui.sections.value[1]); ui.handleDemoteToPrev();
  ui.openPopoverForSection(ui.sections.value[1]); ui.handleDemoteToPrev();
  await ui.saveAndOpenReader();
  const saved = ui.received[1][0];
  const chunks = graph().text.paragraphs(saved).flatMap(group => group.chunks);
  assert.deepEqual(Array.from(chunks, c => c.text), ['Body 0.', 'Body 1.', 'Body 2.']);
  assert.equal(new Set(chunks.map(c => c.id)).size, chunks.length, 'duplicate IDs are rejected by native storage validation');
});
test('progress: flush must wait for an existing write before reporting completion', async () => {
  let complete;
  const pending = new Promise(resolve => { complete = resolve; });
  const { store } = graph(() => pending); store.importNovel(novel());
  store.saveReadingProgress('book', 1, 2, 0.3);
  const first = store.flushReadingProgress();
  let finished = false;
  const second = store.flushReadingProgress().then(() => { finished = true; });
  await Promise.resolve(); await Promise.resolve();
  const finishedEarly = finished;
  complete(); await first; await second;
  assert.equal(finishedEarly, false, 'second flush returned while the first native write was still running');
});
test('progress: failed persistence must remain observable to the reader', async () => {
  const { store } = graph(async () => { throw Error('database busy'); }); store.importNovel(novel());
  store.saveReadingProgress('book', 1, 2, 0.3);
  await assert.rejects(store.flushReadingProgress(), /database busy/);
  assert.notEqual(store.failure.value, '', 'write failure was swallowed and the pending position was discarded');
});
test('progress: reimport must preserve the same-book reading position', async () => {
  const { store } = graph(); const n = novel(); store.importNovel(n);
  store.saveReadingProgress('book', 1, 2, 0.3); store.importNovel(n);
  assert.equal(store.activeBook.value.currentChapterIndex, 1, 'native preserves position but importNovel resets it to zero');
});
test('reader: scrolling to a chapter must update sentence navigation projection', async () => {
  const { store } = graph(); store.importNovel(novel());
  store.saveReadingProgress('book', 1, 0, 0.2);
  assert.equal(store.activeBook.value.sentences[0], 'Body 1.', 'sentence navigation still points to the previous chapter');
});
test('lifecycle: old hydration must not overwrite committed audited sections', async () => {
  let resolveLoad;
  const pending = new Promise(resolve => { resolveLoad = resolve; });
  const { store } = graph(command => command === 'load_novel' ? pending : Promise.resolve({ anomalies: [] }));
  store.books.value = [{ ...novel().book, format: 'TXT', progress: 0, lastRead: '', coverGradient: '', currentChapter: '', currentChapterIndex: 0, sentences: [] }];
  store.activeBookId.value = 'book';
  const loading = store.ensureBookLoaded('book');
  const replacement = [{ ...chapter(0), title: 'Corrected', body: 'New text', chunks: [] }];
  await store.saveAuditedSections('book', replacement, []);
  resolveLoad(novel()); await loading;
  assert.equal(store.activeBook.value.sections[0].title, 'Corrected', 'old load response overwrote a newer successful edit');
});
test('lifecycle: old catalog response must not revive a deleted book', async () => {
  let resolveSummaries;
  const pending = new Promise(resolve => { resolveSummaries = resolve; });
  const { store } = graph(command => command === 'load_book_summaries' ? pending : Promise.resolve());
  store.importNovel(novel()); const loading = store.loadStoredNovels();
  await store.deleteBook('book');
  resolveSummaries([{ ...novel().book, format: 'TXT', first_chapter: 'Chapter 0', last_chapter_index: 0, last_paragraph_index: 0, last_scroll_ratio: 0 }]); await loading;
  assert.equal(store.books.value.length, 0, 'late summary response recreated the deleted shelf entry');
});
test('browser: unsupported saga binary must not be interpreted as TXT', async () => {
  const imported = [];
  const library = { books: vue.ref([]), activeBook: vue.ref(null), isGlobalBusy: vue.ref(false), importBook: (...args) => imported.push(args) };
  const ui = component('Shelf', library, 'processImportedFile', false);
  await ui.processImportedFile({ name: 'sample.saga', text: async () => 'PK\u0003\u0004binary archive' });
  assert.equal(imported.length, 0, 'binary archive was admitted as a successful text import');
});

test('progress: independent books drain through one ordered writer', async () => {
  const calls = [];
  let release;
  const { store } = graph((command, payload) => {
    assert.equal(command, 'update_reading_progress'); calls.push(payload);
    return calls.length === 1 ? new Promise(resolve => { release = resolve; }) : Promise.resolve();
  });
  store.importNovel(novel());
  const second = novel(); second.book.id = 'second'; store.importNovel(second);
  store.saveReadingProgress('book', 1, 0, 0.2);
  const saving = store.flushReadingProgress();
  store.saveReadingProgress('second', 2, 0, 0.4);
  store.saveReadingProgress('book', 2, 0, 0.7);
  assert.equal(calls.length, 1);
  release(); await saving;
  assert.deepEqual(calls.map(item => [item.bookId, item.chapterIndex]), [['book', 1], ['second', 2], ['book', 2]]);
});

test('progress: failure retains the newer queued anchor for explicit retry', async () => {
  let reject;
  const calls = [];
  const { store } = graph((command, payload) => {
    calls.push(payload);
    return calls.length === 1 ? new Promise((resolve, fail) => { reject = fail; }) : Promise.resolve();
  });
  store.importNovel(novel()); store.saveReadingProgress('book', 1, 0, 0.2);
  const saving = store.flushReadingProgress();
  store.saveReadingProgress('book', 2, 0, 0.8);
  reject(Error('busy')); await assert.rejects(saving, /busy/);
  await store.flushReadingProgress();
  assert.deepEqual(calls.map(item => item.chapterIndex), [1, 2]);
  assert.equal(store.failure.value, '');
});

test('progress: reimport during a write queues the clamped replacement anchor', async () => {
  const calls = []; let release;
  const { store } = graph((command, payload) => {
    calls.push(payload);
    return calls.length === 1 ? new Promise(resolve => { release = resolve; }) : Promise.resolve();
  });
  store.importNovel(novel()); store.saveReadingProgress('book', 2, 0, 0.9);
  const saving = store.flushReadingProgress();
  store.importNovel(novel(1)); release(); await saving;
  assert.deepEqual(calls.map(item => item.chapterIndex), [2, 0]);
  assert.equal(calls[1].scrollRatio, 0);
});

test('progress: delete waits for its writer and cancels later queued positions', async () => {
  const calls = []; let release;
  const { store, clock } = graph(command => {
    calls.push(command);
    return command === 'update_reading_progress' ? new Promise(resolve => { release = resolve; }) : Promise.resolve();
  });
  store.importNovel(novel()); store.saveReadingProgress('book', 1, 0, 0.2);
  const writing = store.flushReadingProgress();
  store.saveReadingProgress('book', 2, 0, 0.3);
  const deleting = store.deleteBook('book'); clock.run();
  assert.deepEqual(calls, ['update_reading_progress']);
  release(); await writing; await deleting; await store.flushReadingProgress();
  assert.deepEqual(calls, ['update_reading_progress', 'delete_novel']);
  assert.equal(store.books.value.length, 0);
});

test('progress: rejected deletion leaves its queued position retryable', async () => {
  const calls = [];
  const { store } = graph((command, payload) => {
    calls.push([command, payload]);
    return command === 'delete_novel' ? Promise.reject(Error('locked')) : Promise.resolve();
  });
  store.importNovel(novel()); store.saveReadingProgress('book', 1, 0, 0.2);
  await assert.rejects(store.deleteBook('book'), /locked/);
  await store.flushReadingProgress();
  assert.deepEqual(calls.map(item => item[0]), ['delete_novel', 'update_reading_progress']);
  assert.equal(calls[1][1].chapterIndex, 1);
});

test('lifecycle: obsolete hydration failure cannot overwrite a successful reimport', async () => {
  let reject;
  const { store } = graph(() => new Promise((resolve, fail) => { reject = fail; }));
  store.books.value = [{ ...novel().book, currentChapterIndex: 0, sentences: [] }];
  const loading = store.ensureBookLoaded('book');
  store.importNovel(novel()); reject(Error('old failure')); await loading;
  assert.equal(store.failure.value, '');
});

test('text: lazy sentence projection matches native terminal and quote rules', () => {
  const text = graph().text;
  const section = { ...chapter(0, false), body: '甲！？乙。"\n\nEnglish!? remains together.\n🙂；尾声' };
  assert.deepEqual(Array.from(text.contents(section)), ['甲！', '？', '乙。"', 'English!? remains together.', '🙂；', '尾声']);
  assert.deepEqual(Array.from(text.paragraphs(section).flatMap(group => group.chunks.map(chunk => chunk.text))), Array.from(text.contents(section)));
});

test('reader: an earlier chapter jump cannot publish after selection changes', async () => {
  let release;
  const book = { ...novel().book, sections: novel().sections, currentChapterIndex: 0 };
  const library = { activeBook: vue.ref(book), failure: vue.ref(''), setBookChapter: () => new Promise(resolve => { release = resolve; }), saveReadingProgress: () => assert.fail('stale jump persisted'), flushReadingProgress: async () => { } };
  const ui = component('Reader', library, 'onSelectChapter, renderedChapters');
  const jumping = ui.onSelectChapter(1);
  library.activeBook.value = { ...book, id: 'other' };
  release(); await jumping; await vue.nextTick();
  assert.equal(ui.renderedChapters.value.length, 0);
});

test('reader: offscreen chapter pruning compensates the retained anchor', async () => {
  const { store } = graph(); store.importNovel(novel(12));
  const ui = component('Reader', store, 'trim, renderedChapters, readerViewportRef');
  ui.renderedChapters.value = store.activeBook.value.sections;
  const blocks = Array.from({ length: 12 }, (_, index) => ({ getBoundingClientRect: () => ({ top: index * 500 - 4000, bottom: index * 500 - 3500 }) }));
  const viewport = { scrollTop: 4000, querySelectorAll: () => blocks, getBoundingClientRect: () => ({ top: 0, bottom: 800 }) };
  ui.readerViewportRef.value = viewport;
  ui.trim();
  const first = ui.renderedChapters.value[0].index;
  assert.equal(first, 6);
  for (let index = first; index < blocks.length; index++) blocks[index].getBoundingClientRect = () => ({ top: index * 500 - 4000 - first * 500, bottom: index * 500 - 3500 - first * 500 });
  await vue.nextTick();
  assert.equal(viewport.scrollTop, 1000);
  assert.deepEqual(Array.from(ui.renderedChapters.value, section => section.index), [6, 7, 8, 9, 10, 11]);
});

test('reader: a failed progress flush keeps the return action on the reader', async () => {
  const library = { activeBook: vue.ref(null), failure: vue.ref(''), flushReadingProgress: async () => { throw Error('busy'); } };
  const ui = component('Reader', library, 'handleBackToShelf');
  await ui.handleBackToShelf();
  assert.equal(ui.events.length, 0);
});
