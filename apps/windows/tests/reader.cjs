/** Render actual Vue templates to verify toolbar structure and audio-dependent controls. */
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const { runInNewContext } = require("node:vm");
const { test } = require("node:test");
const ts = require("typescript");
const vue = require("vue");
const { parse, compileScript } = require("vue/compiler-sfc");
const { renderToString } = require("vue/server-renderer");
const loader = require("./support/loader.cjs");

/** Compile the shipped template, supplying only the library and native host boundaries. */
async function render(filename, available) {
  const store = loader()("stores/library.ts");
  store.importNovel({
    book: { id: "book", title: "Text-only.saga", author: "Author", description: "", chapters: 1 },
    sections: [{ id: "chapter", index: 0, title: "Chapter", volume: "Body", body: "Text.", chunks: [] }]
  });
  if (available) store.activeBook.value.audio = [new Blob(["audio"], { type: "audio/mpeg" })];
  const source = readFileSync(join(__dirname, "../src", filename), "utf8");
  const compiled = compileScript(parse(source).descriptor, { id: "reader-test", inlineTemplate: true }).content;
  const exports = {};
  const modules = loader();
  runInNewContext(ts.transpileModule(compiled, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
  }).outputText, {
    exports,
    require(name) {
      if (name === "vue") return vue;
      if (name === "../stores/library") return store;
      if (name === "../text") return modules("text.ts");
      throw new Error("Unexpected rendering dependency: " + name);
    }
  });
  return renderToString(vue.createSSRApp(exports.default));
}

test("reader toolbar renders exactly the three requested icon buttons", async () => {
  const html = await render("views/Reader.vue", false);
  const header = html.match(/<header[^>]*>([\s\S]*?)<\/header>/)[1];
  assert.equal((header.match(/<button\b/g) || []).length, 3);
  assert.equal((header.match(/<svg\b/g) || []).length, 3);
  for (const label of ["返回书架", "打开或关闭章节目录", "阅读设置"]) assert.ok(header.includes('aria-label="' + label + '"'));
  assert.doesNotMatch(html, /reader-footer|reader-delete-btn|system-status|quick-nav-group/);
});

test("both read-along bars render only when the selected book has loaded audio", async () => {
  assert.doesNotMatch(await render("components/Bar.vue", false), /mini-bar/);
  assert.match(await render("components/Bar.vue", true), /class="mini-bar"/);
  assert.match(await render("views/Reader.vue", true), /class="reader-footer"/);
});
