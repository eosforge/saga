/** Check local TypeScript conventions and formatting using the installed compiler. */
const fs = require("node:fs");
const path = require("node:path");
const ts = require("typescript");

// The compiler owns script formatting; templates and CSS remain outside this check.
const root = path.resolve(__dirname, "..");
const options = {
  indentSize: 2, tabSize: 2, convertTabsToSpaces: true, newLineCharacter: "\n",
  insertSpaceAfterCommaDelimiter: true, insertSpaceAfterSemicolonInForStatements: true,
  insertSpaceBeforeAndAfterBinaryOperators: true, insertSpaceAfterKeywordsInControlFlowStatements: true,
  insertSpaceAfterFunctionKeywordForAnonymousFunctions: true, insertSpaceAfterOpeningAndBeforeClosingNonemptyBraces: true,
  placeOpenBraceOnNewLineForFunctions: false, placeOpenBraceOnNewLineForControlBlocks: false
};

/** Enumerate maintained scripts without entering installed or generated directories. */
function files(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const filename = path.join(directory, entry.name);
    return entry.isDirectory() ? files(filename) : /\.(ts|vue|cjs)$/.test(entry.name) ? [filename] : [];
  });
}

/** Apply the compiler's edits backwards so original offsets remain valid. */
function format(source) {
  const filename = "source.ts";
  const service = ts.createLanguageService({
    getScriptFileNames: () => [filename], getScriptVersion: () => "0",
    getScriptSnapshot: name => name === filename ? ts.ScriptSnapshot.fromString(source) : undefined,
    getCompilationSettings: () => ({}), getCurrentDirectory: () => root,
    getDefaultLibFileName: () => "lib.d.ts", fileExists: () => false, readFile: () => undefined
  });
  const edits = service.getFormattingEditsForDocument(filename, options);
  for (const edit of edits.sort((left, right) => right.span.start - left.span.start)) {
    source = source.slice(0, edit.span.start) + edit.newText + source.slice(edit.span.start + edit.span.length);
  }
  service.dispose();
  return source;
}

/** Report named TypeScript operations whose result contract is implicit. */
function returns(source, filename) {
  const errors = [];
  const syntax = ts.createSourceFile(filename, source, ts.ScriptTarget.Latest, true);
  function visit(node) {
    const named = ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node) ||
      (ts.isArrowFunction(node) && ts.isVariableDeclaration(node.parent));
    if (named && !node.type) errors.push("missing return type at " + syntax.getLineAndCharacterOfPosition(node.getStart()).line);
    ts.forEachChild(node, visit);
  }
  visit(syntax);
  return errors;
}

/** Check by default; --write performs only compiler-owned script formatting. */
function main() {
  const write = process.argv.slice(2).join(" ") === "--write";
  if (process.argv.length > 2 && !write) throw new Error("Usage: node tools/style.cjs [--write]");
  let failures = 0;
  const targets = [...files(path.join(root, "src")), path.join(root, "vite.config.ts")];
  for (const filename of targets) {
    const original = fs.readFileSync(filename, "utf8");
    const normalized = original.replace(/\r\n/g, "\n");
    const match = filename.endsWith(".vue") ? normalized.match(/<script setup lang="ts">([\s\S]*?)<\/script>/) : null;
    const source = match ? match[1] : normalized;
    const formatted = format(source);
    const replacement = (match ? normalized.replace(match[0], '<script setup lang="ts">' + formatted + "</script>") : formatted).trimEnd() + "\n";
    const errors = returns(source, filename);
    if (filename.endsWith(".vue") && !/^[A-Z]/.test(path.basename(filename))) errors.push("component filename must use PascalCase");
    if (original !== replacement) {
      if (write) fs.writeFileSync(filename, replacement);
      else errors.push("script formatting differs; run pnpm format");
    }
    for (const error of errors) { console.error(path.relative(root, filename) + ": " + error); failures++; }
  }
  console.log("Checked " + targets.length + " handwritten TypeScript/Vue scripts with TypeScript " + ts.version);
  if (failures) process.exitCode = 1;
}

main();
