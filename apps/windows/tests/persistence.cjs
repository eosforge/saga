/** Verify real progress scheduling, configurable intervals, native lifecycle hooks, and export ordering. */
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { resolve, dirname } = require('node:path');
const { runInNewContext } = require('node:vm');
const { test } = require('node:test');
const ts = require('typescript');
const vue = require('vue');

/** Drain asynchronous module continuations without waiting for real business time. */
async function settle() {
  for (let index = 0; index < 16; index++) await Promise.resolve();
}

/** Expose an explicitly controlled IPC or listener completion. */
function deferred() {
  let resolve, reject;
  const promise = new Promise((success, failure) => { resolve = success; reject = failure; });
  return { promise, resolve, reject };
}

/** Keep event ownership observable, including removal and late callbacks. */
function surface() {
  const handlers = new Map();
  return {
    handlers,
    addEventListener(name, callback) { if (!handlers.has(name)) handlers.set(name, new Set()); handlers.get(name).add(callback); },
    removeEventListener(name, callback) { handlers.get(name)?.delete(callback); },
    emit(name) { for (const callback of handlers.get(name) || []) callback(); },
  };
}

/** Run actual modules with virtual monotonic time and isolated native/browser boundaries. */
function fixture(options = {}) {
  const cache = new Map(), timers = new Map(), calls = [], storage = options.storage || new Map();
  const document = { ...surface(), visibilityState: 'visible', activeElement: null };
  const window = surface();
  let elapsed = 0, serial = 0, destructions = 0, releases = 0;
  const callbacks = {};
  const host = {
    onCloseRequested(callback) { callbacks.close = callback; return options.closeRegistration?.promise || Promise.resolve(() => { releases++; }); },
    onFocusChanged(callback) { callbacks.focus = callback; return options.focusRegistration?.promise || Promise.resolve(() => { releases++; }); },
    async destroy() { if (options.destroy) await options.destroy(); destructions++; },
  };
  class Input { constructor(value) { this.value = String(value); } }
  const boundaries = {
    console, Blob, crypto: require('node:crypto').webcrypto, document, window, HTMLInputElement: Input,
    performance: { now: () => elapsed },
    localStorage: {
      getItem(key) { if (options.inaccessible) throw Error('storage blocked'); return storage.get(key) ?? null; },
      setItem(key, value) { if (options.inaccessible) throw Error('storage blocked'); storage.set(key, value); },
    },
    setTimeout(callback, delay) { timers.set(++serial, { callback, at: elapsed + delay }); return serial; },
    clearTimeout(id) { timers.delete(id); },
    setInterval() { throw Error('unexpected playback'); }, clearInterval() {},
  };
  function load(relative, exposed = '') {
    const filename = resolve(__dirname, '../src', relative);
    if (!exposed && cache.has(filename)) return cache.get(filename);
    const exports = {}, lifecycle = {};
    if (!exposed) cache.set(filename, exports);
    const source = readFileSync(filename, 'utf8');
    const script = filename.endsWith('.vue') ? source.match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1] : source;
    const code = ts.transpileModule(script + (exposed ? '\nexport { ' + exposed + ' };' : ''), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    }).outputText;
    runInNewContext(code, {
      ...boundaries, exports,
      require(name) {
        if (name === 'vue') return { ...vue, onMounted: callback => { lifecycle.mount = callback; }, onUnmounted: callback => { lifecycle.unmount = callback; } };
        if (name === '@tauri-apps/api/core') return { isTauri: () => options.native !== false, invoke: async (command, payload) => {
          calls.push({ command, payload }); return options.invoke?.(command, payload);
        } };
        if (name === '@tauri-apps/api/window') return { getCurrentWindow: () => host };
        if (name.endsWith('.vue')) return {};
        if (name.startsWith('.')) return load(resolve(dirname(filename), name + '.ts'));
        throw Error(name);
      },
    });
    return exposed ? { ...exports, ...lifecycle, source } : exports;
  }
  const store = load('stores/library.ts');
  store.importNovel({ book: { id: 'book', title: 'Fixture', author: 'Author', description: '', chapters: 3 }, sections: [0, 1, 2].map(index => ({
    id: 'book:' + index, index, title: 'Chapter ' + index, volume: 'Volume', body: 'First paragraph.\nSecond paragraph.', chunks: [],
  })) });
  return {
    store, calls, storage, callbacks, document, window, timers, Input, load,
    get destructions() { return destructions; }, get releases() { return releases; },
    async advance(milliseconds) {
      const end = elapsed + milliseconds;
      while (true) {
        const next = [...timers].filter(([, timer]) => timer.at <= end).sort((left, right) => left[1].at - right[1].at)[0];
        if (!next) break;
        elapsed = next[1].at; timers.delete(next[0]); next[1].callback(); await settle();
      }
      elapsed = end; await settle();
    },
  };
}

test('continuous scrolling saves the latest anchor at 30 seconds without resetting the deadline', async () => {
  const ui = fixture();
  for (let second = 0; second < 30; second++) {
    ui.store.saveReadingProgress('book', 1, 0, second / 100);
    await ui.advance(999);
    assert.equal(ui.calls.length, 0);
    await ui.advance(1);
  }
  assert.equal(ui.calls.length, 1);
  assert.equal(ui.calls[0].payload.scrollRatio, 0.29);
  ui.store.saveReadingProgress('book', 1, 0, 0.29);
  await ui.advance(60_000);
  assert.equal(ui.calls.length, 1);
  assert.equal(ui.timers.size, 0);
});

test('all five save intervals persist and unsupported stored values fall back to 30 seconds', async () => {
  const storage = new Map([['saga_progress_interval', 'invalid']]);
  const ui = fixture({ storage });
  assert.equal(ui.store.saveInterval.value, 30);
  assert.deepEqual(Array.from(ui.store.saveIntervals), [5, 10, 15, 30, 60]);
  for (const seconds of [5, 10, 15, 30, 60]) {
    ui.store.configureSaving(seconds);
    assert.equal(fixture({ storage }).store.saveInterval.value, seconds);
  }
  ui.store.configureSaving(7);
  assert.equal(ui.store.saveInterval.value, 60);
  ui.store.saveReadingProgress('book', 1, 0, 0.5);
  await ui.advance(59_999); assert.equal(ui.calls.length, 0);
  await ui.advance(1); assert.equal(ui.calls.length, 1);
});

test('changing the interval accounts for time already spent waiting', async () => {
  const ui = fixture();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  await ui.advance(9_000);
  ui.store.configureSaving(5);
  await ui.advance(0);
  assert.equal(ui.calls.length, 1);
  ui.store.saveReadingProgress('book', 1, 0, 0.4);
  await ui.advance(2_000);
  ui.store.configureSaving(15);
  await ui.advance(12_999); assert.equal(ui.calls.length, 1);
  await ui.advance(1); assert.equal(ui.calls.length, 2);
});

test('unavailable webview preference storage keeps the reader usable and applies the session choice', async () => {
  const ui = fixture({ inaccessible: true });
  assert.equal(ui.store.saveInterval.value, 30);
  ui.store.configureSaving(5);
  assert.equal(ui.store.saveInterval.value, 5);
  assert.match(ui.store.failure.value, /本次会话/);
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  await ui.advance(5_000);
  assert.equal(ui.calls.length, 1);
});

test('returning to an acknowledged anchor cancels a dirty write but cannot bypass a newer in-flight write', async () => {
  const pending = deferred();
  const ui = fixture({ invoke: () => ui.calls.length === 2 ? pending.promise : undefined });
  ui.store.saveReadingProgress('book', 1, 0, 0.2); await ui.store.flushReadingProgress();
  ui.store.saveReadingProgress('book', 1, 0, 0.4);
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  await ui.advance(30_000); assert.equal(ui.calls.length, 1);
  ui.store.saveReadingProgress('book', 1, 0, 0.6);
  const flushing = ui.store.flushReadingProgress();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  pending.resolve(); await flushing;
  assert.deepEqual(ui.calls.map(call => call.payload.scrollRatio), [0.2, 0.6, 0.2]);
});

test('an unknown write outcome invalidates the older acknowledgement before retry', async () => {
  const ui = fixture({ invoke: () => { if (ui.calls.length === 2) throw Error('response lost after commit'); } });
  ui.store.saveReadingProgress('book', 1, 0, 0.2); await ui.store.flushReadingProgress();
  ui.store.saveReadingProgress('book', 1, 0, 0.6);
  await assert.rejects(ui.store.flushReadingProgress(), /response lost/);
  ui.store.saveReadingProgress('book', 1, 0, 0.2); await ui.store.flushReadingProgress();
  assert.deepEqual(ui.calls.map(call => call.payload.scrollRatio), [0.2, 0.6, 0.2]);
});

test('application focus loss, hidden visibility and pagehide flush early without duplicate writes', async () => {
  const ui = fixture(), app = ui.load('App.vue', 'currentTab');
  app.mount(); await settle();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  ui.callbacks.focus({ payload: false }); await settle();
  assert.equal(ui.calls.length, 1);
  ui.store.saveReadingProgress('book', 1, 0, 0.4);
  ui.document.visibilityState = 'hidden'; ui.document.emit('visibilitychange'); await settle();
  assert.equal(ui.calls.length, 2);
  ui.window.emit('pagehide'); await settle(); assert.equal(ui.calls.length, 2);
  app.unmount(); await settle();
  assert.equal(ui.releases, 2);
  assert.equal(ui.document.handlers.get('visibilitychange').size, 0);
  assert.equal(ui.window.handlers.get('pagehide').size, 0);
});

test('normal close waits for the latest position and repeated requests destroy the window only once', async () => {
  const pending = deferred();
  const ui = fixture({ invoke: () => ui.calls.length === 1 ? pending.promise : undefined });
  const app = ui.load('App.vue', 'currentTab, handleGlobalKeydown'); app.mount(); await settle();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  let prevented = 0;
  const event = { preventDefault() { prevented++; } };
  const closing = ui.callbacks.close(event);
  await ui.callbacks.close(event);
  assert.equal(ui.destructions, 0);
  assert.equal(ui.load('lifecycle.ts').closing.value, true);
  let blocked = 0;
  app.handleGlobalKeydown({ preventDefault() { blocked++; }, stopImmediatePropagation() { blocked++; } });
  assert.equal(blocked, 2);
  ui.store.saveReadingProgress('book', 2, 1, 0.8);
  pending.resolve(); await closing;
  assert.equal(prevented, 2);
  assert.equal(ui.destructions, 1);
  assert.deepEqual(ui.calls.map(call => call.payload.chapterIndex), [1, 2]);
  app.unmount();
});

test('failed close keeps the window open and the same request can retry saving', async () => {
  let reject = true;
  const ui = fixture({ invoke: () => { if (reject) throw Error('disk unavailable'); } });
  const app = ui.load('App.vue', 'currentTab'); app.mount(); await settle();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  const event = { preventDefault() {} };
  await ui.callbacks.close(event);
  assert.equal(ui.destructions, 0);
  assert.match(ui.store.failure.value, /窗口已保留/);
  assert.equal(ui.load('lifecycle.ts').closing.value, false);
  reject = false; await ui.callbacks.close(event);
  assert.equal(ui.calls.length, 2); assert.equal(ui.destructions, 1);
  app.unmount();
});

test('mouse back and forward cannot navigate while close waits for persistence', async () => {
  const pending = deferred();
  const ui = fixture({ invoke: () => pending.promise });
  const app = ui.load('App.vue', 'currentTab, switchTab, handleMouseNavigation');
  app.mount(); await settle();
  app.switchTab('reader');
  app.switchTab('settings');
  app.handleMouseNavigation({ button: 3, preventDefault() {}, stopPropagation() {} });
  assert.equal(app.currentTab.value, 'reader');
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  const closing = ui.callbacks.close({ preventDefault() {} });
  let blocked = 0;
  for (const button of [3, 4]) {
    app.handleMouseNavigation({ button, preventDefault() { blocked++; }, stopPropagation() { blocked++; } });
    assert.equal(app.currentTab.value, 'reader');
  }
  assert.equal(blocked, 4);
  pending.resolve(); await closing;
  assert.equal(ui.destructions, 1);
  app.unmount();
});

test('close stays available after a native destroy failure and does not repeat an acknowledged progress write', async () => {
  let reject = true;
  const ui = fixture({ destroy: () => { if (reject) throw Error('window busy'); } });
  const app = ui.load('App.vue', 'currentTab'); app.mount(); await settle();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  const event = { preventDefault() {} };
  await ui.callbacks.close(event);
  assert.equal(ui.destructions, 0);
  assert.match(ui.store.failure.value, /阅读进度已保存/);
  reject = false; await ui.callbacks.close(event);
  assert.equal(ui.calls.length, 1); assert.equal(ui.destructions, 1);
  app.unmount();
});

test('a later successful background save clears the earlier close-save failure', async () => {
  let reject = true;
  const ui = fixture({ invoke: () => { if (reject) throw Error('disk unavailable'); } });
  const app = ui.load('App.vue', 'currentTab'); app.mount(); await settle();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  await ui.callbacks.close({ preventDefault() {} });
  assert.match(ui.store.failure.value, /关闭前保存失败/);
  reject = false; ui.callbacks.focus({ payload: false }); await settle();
  assert.equal(ui.store.failure.value, '');
  assert.equal(ui.destructions, 0);
  app.unmount();
});

test('close does not terminate an active book operation', async () => {
  const ui = fixture(), app = ui.load('App.vue', 'currentTab');
  app.mount(); await settle(); ui.store.isGlobalBusy.value = true;
  await ui.callbacks.close({ preventDefault() {} });
  assert.equal(ui.destructions, 0); assert.equal(ui.calls.length, 0);
  assert.match(ui.store.failure.value, /尚未完成/);
  app.unmount();
});

test('teardown owns late native registrations and does not close a replaced application view', async () => {
  const closeRegistration = deferred(), focusRegistration = deferred(), pending = deferred();
  const ui = fixture({ closeRegistration, focusRegistration, invoke: () => pending.promise });
  const app = ui.load('App.vue', 'currentTab'); app.mount();
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  const closing = ui.callbacks.close({ preventDefault() {} });
  app.unmount();
  assert.equal(ui.load('lifecycle.ts').closing.value, false);
  let released = 0;
  closeRegistration.resolve(() => { released++; }); focusRegistration.resolve(() => { released++; });
  pending.resolve(); await closing; await settle();
  assert.equal(released, 2); assert.equal(ui.destructions, 0);
});

test('settings slider selects discrete seconds and export waits for a successful progress flush', async () => {
  const pending = deferred();
  const ui = fixture({ invoke: command => command === 'update_reading_progress' ? pending.promise : 'chosen/library.json' });
  const settings = ui.load('views/Settings.vue', 'onInterval, onExport, exportStatus, exporting');
  settings.onInterval({ target: new ui.Input(4) });
  assert.equal(ui.store.saveInterval.value, 60);
  settings.onInterval({ target: new ui.Input(1) });
  assert.equal(ui.store.saveInterval.value, 10);
  assert.match(settings.source, /aria-valuetext/);
  ui.store.saveReadingProgress('book', 1, 0, 0.2);
  const exporting = settings.onExport();
  await settings.onExport();
  assert.equal(ui.calls.length, 1);
  pending.resolve(); await exporting;
  assert.deepEqual(ui.calls.map(call => call.command), ['update_reading_progress', 'export_library_state']);
  assert.match(settings.exportStatus.value, /chosen\/library.json/);
  assert.equal(settings.exporting.value, false);
});

test('export cancellation is reported and a failed progress save never opens the destination picker', async () => {
  const cancelled = fixture({ invoke: () => null });
  const settings = cancelled.load('views/Settings.vue', 'onExport, exportStatus');
  await settings.onExport(); assert.match(settings.exportStatus.value, /已取消/);
  const failed = fixture({ invoke: () => { throw Error('disk unavailable'); } });
  const rejected = failed.load('views/Settings.vue', 'onExport, exportStatus');
  failed.store.saveReadingProgress('book', 1, 0, 0.2);
  await rejected.onExport();
  assert.deepEqual(failed.calls.map(call => call.command), ['update_reading_progress']);
  assert.match(rejected.exportStatus.value, /导出失败/);
});
