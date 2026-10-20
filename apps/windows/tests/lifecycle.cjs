/** Verify keyboard ownership and asynchronous reader teardown using real component scripts. */
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const { runInNewContext } = require("node:vm");
const { test } = require("node:test");
const ts = require("typescript");
const vue = require("vue");

/** Build observable browser boundaries without mounting a real desktop window. */
function fixture(loading = Promise.resolve()) {
  const handlers = new Map();
  let toggles = 0;
  const document = { activeElement: null };
  const window = {
    addEventListener(name, callback) { handlers.set(name, [...(handlers.get(name) || []), callback]); },
    removeEventListener(name, callback) { handlers.set(name, (handlers.get(name) || []).filter(item => item !== callback)); }
  };
  const library = {
    activeBook: vue.ref({ id: "book", sections: [], currentChapterIndex: 0 }),
    failure: vue.ref(""), activeSentenceIndex: vue.ref(0), isPlaying: vue.ref(false), hasAudio: vue.ref(true),
    ensureBookLoaded: () => loading, togglePlay: () => { toggles++; }, pause() {}
  };
  function component(filename, exposed) {
    const source = readFileSync(join(__dirname, "../src", filename), "utf8").match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1];
    const code = ts.transpileModule(source + "\nexport { " + exposed + " };", {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
    }).outputText;
    const lifecycle = {};
    const exports = {};
    runInNewContext(code, {
      exports, window, document, defineEmits: () => () => {},
      require(name) {
        if (name === "vue") return { ...vue, watch() {}, onMounted: callback => { lifecycle.mount = callback; }, onUnmounted: callback => { lifecycle.unmount = callback; } };
        if (name.endsWith("stores/library")) return library;
        if (name.endsWith(".vue")) return {};
        if (name === "../text") return require("./support/loader.cjs")()("text.ts");
        if (name === "@tauri-apps/api/core") return { isTauri: () => true, invoke: async () => "pong" };
        throw new Error("Unexpected dependency: " + name);
      }
    });
    return { ...exports, ...lifecycle };
  }
  return { component, handlers, document, library, get toggles() { return toggles; } };
}

test("a reader Space press reaches one owner and focused controls keep their keys", async () => {
  const ui = fixture();
  const app = ui.component("App.vue", "currentTab");
  const reader = ui.component("views/Reader.vue", "handleKeydown");
  app.currentTab.value = "reader";
  app.mount();
  await reader.mount();
  const event = { code: "Space", preventDefault() {} };
  for (const callback of ui.handlers.get("keydown")) callback(event);
  assert.equal(ui.toggles, 1);
  ui.document.activeElement = { matches: () => true };
  for (const callback of ui.handlers.get("keydown")) callback(event);
  assert.equal(ui.toggles, 1);
  reader.unmount();
  app.unmount();
  assert.equal(ui.handlers.get("keydown").length, 0);
});

test("Space remains normal scrolling when the selected book has no audio", async () => {
  const ui = fixture();
  ui.library.hasAudio.value = false;
  const app = ui.component("App.vue", "currentTab");
  const reader = ui.component("views/Reader.vue", "handleKeydown");
  app.mount();
  await reader.mount();
  const event = { code: "Space", preventDefault() { assert.fail("text-only Space must remain available to the browser"); } };
  for (const callback of ui.handlers.get("keydown")) callback(event);
  assert.equal(ui.toggles, 0);
  reader.unmount();
  app.unmount();
});

test("reader hydration completing after unmount does not leak a keyboard listener", async () => {
  let complete;
  const ui = fixture(new Promise(resolve => { complete = resolve; }));
  const reader = ui.component("views/Reader.vue", "handleKeydown");
  const mounting = reader.mount();
  reader.unmount();
  complete();
  await mounting;
  assert.equal(ui.handlers.get("keydown").length, 0);
  assert.equal(ui.handlers.get("pointerdown").length, 0);
});

test("reading settings own their keys, focus the range, and dismiss with Escape or an outside click", async () => {
  const ui = fixture();
  const reader = ui.component("views/Reader.vue", "settings, controls, panel, trigger, configure, handleKeydown, dismiss");
  let focused = 0;
  reader.panel.value = { querySelector: () => ({ focus() { focused++; } }) };
  reader.trigger.value = { focus() { focused++; } };
  reader.controls.value = { contains: target => target === "inside" };
  await reader.configure();
  assert.equal(reader.settings.value, true);
  assert.equal(focused, 1);
  reader.handleKeydown({ code: "Space", preventDefault() { assert.fail("settings preserve control keys"); } });
  assert.equal(ui.toggles, 0);
  reader.dismiss({ target: "inside" });
  assert.equal(reader.settings.value, true);
  reader.handleKeydown({ code: "Escape", preventDefault() {} });
  assert.equal(reader.settings.value, false);
  assert.equal(focused, 2);
  await reader.configure();
  reader.dismiss({ target: "outside" });
  assert.equal(reader.settings.value, false);
});
