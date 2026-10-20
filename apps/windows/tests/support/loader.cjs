/** Load actual TypeScript modules with isolated, explicitly supplied host dependencies. */
const { readFileSync } = require("node:fs");
const { resolve, dirname } = require("node:path");
const { runInNewContext } = require("node:vm");
const ts = require("typescript");
const vue = require("vue");

/** Create one module graph per fixture; imports share state only inside that fixture. */
module.exports = function loader(core = {}) {
  const cache = new Map();
  function load(path) {
    const filename = resolve(__dirname, "../../src", path);
    if (cache.has(filename)) return cache.get(filename);
    const exports = {};
    cache.set(filename, exports);
    const code = ts.transpileModule(readFileSync(filename, "utf8"), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
    }).outputText;
    runInNewContext(code, {
      exports, console, crypto: require("node:crypto").webcrypto, Blob, setInterval, clearInterval,
      require(name) {
        if (name === "vue") return vue;
        if (name === "@tauri-apps/api/core") return { isTauri: () => true, ...core };
        if (name.startsWith(".")) return load(resolve(dirname(filename), name + ".ts"));
        throw new Error("Unexpected dependency: " + name);
      }
    });
    return exports;
  }
  return load;
};
