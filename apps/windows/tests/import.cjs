const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { test } = require('node:test');
const { runInNewContext } = require('node:vm');
const ts = require('typescript');
const vue = require('vue');

// Exercise the component's actual handlers with deterministic IPC and lifecycle boundaries.
function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function fixture(invoke, subscription) {
  const source = readFileSync(join(__dirname, '../src/views/Shelf.vue'), 'utf8');
  const script = source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1];
  const code = ts.transpileModule(`${script}\nexport { triggerFileImport, importFilePath, handleImportResult, isImporting, isPicking, importPercent, importMeasured };`, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const lifecycle = {};
  const events = [];
  const books = [];
  let listener;
  let unsubscribed = 0;
  const exports = {};
  runInNewContext(code, {
    exports,
    console: { warn() {}, error() {} },
    defineEmits: () => event => events.push(event),
    setTimeout: () => 0,
    clearTimeout() {},
    require(name) {
      if (name === 'vue') return {
        ...vue,
        onMounted: callback => { lifecycle.mount = callback; },
        onUnmounted: callback => { lifecycle.unmount = callback; },
      };
      if (name === '@tauri-apps/api/core') return { invoke, isTauri: () => true };
      if (name === '@tauri-apps/api/event') return {
        listen: async (_, callback) => {
          listener = callback;
          if (subscription) await subscription.promise;
          return () => { unsubscribed++; };
        },
      };
      if (name === '../stores/library') return {
        books: vue.ref([]), activeBook: vue.ref(null),
        activeAuditReport: vue.ref(null), activeAuditNovel: vue.ref(null),
        importNovel: novel => books.push(novel),
      };
      throw new Error(`Unexpected dependency: ${name}`);
    },
  });
  return {
    ...exports, lifecycle, events, books,
    get unsubscribed() { return unsubscribed; },
    progress(phase, percent) {
      listener({ payload: { phase, percent, title: phase, detail: phase } });
    },
  };
}

const result = { novel: { book: { title: 'Fixture' }, sections: [] }, validation: { anomalies: [] } };

test('retained findings do not force audit again while pending findings do', () => {
  const ui = fixture(async () => result);
  ui.handleImportResult({ ...result, validation: { anomalies: [{ retained: true }] } });
  assert.deepEqual(ui.events, ['open-reader']);
  ui.handleImportResult({ ...result, validation: { anomalies: [{ retained: true }, { retained: false }] } });
  assert.deepEqual(ui.events, ['open-reader', 'open-audit']);
});

test('picker cancellation does not start import; repeated clicks open one picker', async () => {
  const picker = deferred();
  const calls = [];
  const ui = fixture(command => { calls.push(command); return picker.promise; });
  const first = ui.triggerFileImport();
  await ui.triggerFileImport();
  assert.equal(ui.isPicking.value, true);
  assert.equal(ui.isImporting.value, false);
  assert.deepEqual(calls, ['pick_novel_file']);
  picker.resolve(null);
  await first;
  assert.equal(ui.isPicking.value, false);
  assert.equal(ui.isImporting.value, false);
  assert.equal(ui.books.length, 0);
});

test('listener precedes import; measured progress starts at zero and completion waits for result', async () => {
  const subscription = deferred();
  const imported = deferred();
  const started = deferred();
  const calls = [];
  const ui = fixture(command => {
    calls.push(command);
    assert.equal(ui.importPercent.value, 0);
    started.resolve();
    return imported.promise;
  }, subscription);
  const operation = ui.importFilePath('fixture.txt');
  await ui.importFilePath('second.txt');
  await Promise.resolve();
  assert.equal(calls.length, 0);
  subscription.resolve();
  await started.promise;
  assert.deepEqual(calls, ['import_txt_file']);
  ui.progress('reading', 10);
  assert.equal(ui.importPercent.value, 10);
  assert.equal(ui.importMeasured.value, true);
  ui.progress('hashing', 20);
  assert.equal(ui.importMeasured.value, false);
  ui.progress('parsing', 55);
  assert.equal(ui.importMeasured.value, true);
  ui.progress('parsing', 40);
  assert.equal(ui.importPercent.value, 55);
  ui.progress('completed', 100);
  assert.equal(ui.importPercent.value, 99);
  assert.equal(ui.isImporting.value, true);
  imported.resolve(result);
  await operation;
  assert.equal(ui.importPercent.value, 100);
  assert.equal(ui.isImporting.value, false);
  assert.equal(ui.books.length, 1);
  ui.progress('reading', 5);
  assert.equal(ui.isImporting.value, false);
  assert.equal(ui.importPercent.value, 100);
});

test('picker errors do not fall back to a second dialog or start an import', async () => {
  const calls = [];
  const ui = fixture(async command => { calls.push(command); throw new Error('picker failed'); });
  await ui.triggerFileImport();
  assert.deepEqual(calls, ['pick_novel_file']);
  assert.equal(ui.isPicking.value, false);
  assert.equal(ui.isImporting.value, false);
});

test('failed import clears the overlay and cannot report successful completion', async () => {
  const ui = fixture(async () => { throw new Error('read failed'); });
  await ui.importFilePath('missing.txt');
  assert.equal(ui.isImporting.value, false);
  assert.equal(ui.importPercent.value, 0);
  assert.equal(ui.books.length, 0);
  assert.deepEqual(ui.events, []);
});

test('unmount during listener registration releases the listener and prevents import', async () => {
  const subscription = deferred();
  let calls = 0;
  const ui = fixture(async () => { calls++; return result; }, subscription);
  const operation = ui.importFilePath('fixture.txt');
  ui.lifecycle.unmount();
  subscription.resolve();
  await operation;
  assert.equal(calls, 0);
  assert.equal(ui.unsubscribed, 1);
  assert.equal(ui.isImporting.value, false);
});

test('reimport replaces the matching shelf item and preserves other books', () => {
  const store = require("./support/loader.cjs")()("stores/library.ts");
  const novel = count => ({
    book: { id: 'fixture', title: 'Fixture', author: 'Author', description: '', chapters: count },
    sections: Array.from({ length: count }, (_, index) => ({ id: `fixture:${index}`, index, title: `Chapter ${index}`, volume: 'Volume', body: 'Text', chunks: [] })),
  });
  store.importNovel(novel(4));
  store.importNovel({ ...novel(1), book: { ...novel(1).book, id: 'other' } });
  store.importNovel(novel(2));
  assert.deepEqual(Array.from(store.books.value, book => book.id), ['fixture', 'other']);
  assert.equal(store.books.value[0].sections.length, 2);
});
