const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { test } = require('node:test');
const { runInNewContext } = require('node:vm');
const ts = require('typescript');
const vue = require('vue');

// Run the actual component handlers with an observable heading-only scroll target.
function fixture({ saved, accepted = false } = {}) {
  const original = Array.from({ length: 12 }, (_, index) => ({
    id: `book:${index}`, index, title: index === 7 || index === 8 ? '第8章 同名标题' : `第${index + 1}章 标题`,
    volume: '第一卷', body: `这是第${index}个位置的独立正文，不应因保留而改变。`, chunks: [],
  }));
  const finding = { key: 'fingerprint', retained: accepted, anomaly_type: 'DuplicateHeading', section_index: 8, title: original[8].title, volume: '第一卷', description: '重复章号' };
  const selected = [];
  const library = {
    activeAuditNovel: vue.ref({ book: { id: 'book', title: 'Fixture' }, sections: original }),
    activeAuditReport: vue.ref({ anomalies: [finding] }), activeBook: vue.ref(null),
    saveAuditedSections: saved || (async () => ({})), selectBook: identity => selected.push(identity),
  };
  const script = readFileSync(join(__dirname, '../src/views/Audit.vue'), 'utf8').match(/<script setup lang="ts">([\s\S]*?)<\/script>/)[1];
  const exposed = 'sections, anomalies, resolvedMap, filteredAnomalies, filterType, renderedSections, readerPaperRef, activePopoverSection, editTitle, failure, isSaving, openPopoverForSection, retain, restore, handleUndo, handleApplyRename, handleDeleteSection, selectAnomaly, saveAndOpenReader, skipAndOpenReader, isAnomalySection';
  const code = ts.transpileModule(`${script}\nexport { ${exposed} };`, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const exports = {};
  const events = [];
  const scrolled = [];
  let mount;
  runInNewContext(code, {
    exports, console: { error() {} }, defineEmits: () => event => events.push(event),
    require(name) {
      if (name === 'vue') return { ...vue, onMounted: callback => { mount = callback; }, watch() {} };
      if (name === '../stores/library') return library;
      throw Error(`Unexpected dependency: ${name}`);
    },
  });
  exports.readerPaperRef.value = {
    querySelectorAll: () => exports.renderedSections.value.map(section => ({
      dataset: { sectionId: section.id },
      scrollIntoView: () => assert.fail('must target the heading, not the entire chapter'),
      querySelector: selector => {
        assert.equal(selector, 'h2, .novel-suspicious-heading-line');
        return { scrollIntoView: options => {
          assert.equal(options.behavior, 'auto');
          scrolled.push({ id: section.id, block: options.block });
        } };
      },
    })),
  };
  mount();
  return { ...exports, original, library, events, scrolled, selected };
}

test('retain changes only review state, removes the pending item, and can be undone', async () => {
  const ui = fixture();
  await vue.nextTick();
  assert.equal(ui.isAnomalySection(ui.sections.value[7]), false);
  assert.equal(ui.isAnomalySection(ui.sections.value[8]), true);
  ui.openPopoverForSection(ui.sections.value[8]);
  ui.retain();
  assert.equal(ui.filteredAnomalies.value.length, 0);
  assert.equal(ui.resolvedMap.value[0].label, '已保留');
  assert.equal(JSON.stringify(ui.sections.value), JSON.stringify(ui.original));
  ui.filterType.value = 'RESOLVED';
  assert.equal(ui.filteredAnomalies.value.length, 1);
  ui.handleUndo();
  ui.filterType.value = 'PENDING';
  assert.equal(ui.filteredAnomalies.value.length, 1);
});

test('loaded retain decisions can be reviewed and revoked without changing the chapter', () => {
  const ui = fixture({ accepted: true });
  assert.equal(ui.filteredAnomalies.value.length, 0);
  ui.openPopoverForSection(ui.sections.value[8]);
  ui.restore();
  assert.equal(ui.filteredAnomalies.value.length, 1);
  assert.equal(JSON.stringify(ui.sections.value), JSON.stringify(ui.original));
});

test('navigation targets the exact heading and undo uses chapter position, not finding ordinal', async () => {
  const ui = fixture();
  await vue.nextTick();
  assert.deepEqual(ui.scrolled.at(-1), { id: 'book:8', block: 'start' });
  ui.openPopoverForSection(ui.sections.value[8]);
  ui.editTitle.value = '第8章 新标题';
  ui.handleApplyRename();
  assert.equal(ui.original[8].title, '第8章 同名标题');
  ui.handleUndo();
  await vue.nextTick();
  assert.equal(ui.sections.value[8].title, '第8章 同名标题');
  assert.equal(ui.renderedSections.value[0].id, 'book:7');
  assert.deepEqual(ui.scrolled.at(-1), { id: 'book:8', block: 'start' });
});

test('deleted findings do not fall back to another chapter at the old array position', async () => {
  const ui = fixture();
  await vue.nextTick();
  ui.openPopoverForSection(ui.sections.value[8]);
  ui.handleDeleteSection();
  const count = ui.scrolled.length;
  ui.selectAnomaly(0);
  await vue.nextTick();
  assert.equal(ui.scrolled.length, count);
  assert.equal(ui.original.length, 12);
  ui.handleUndo();
  await vue.nextTick();
  assert.equal(ui.sections.value[8].id, 'book:8');
  assert.deepEqual(ui.scrolled.at(-1), { id: 'book:8', block: 'start' });
});

test('save sends retain keys and waits for persistence; failure stays editable and visible', async () => {
  let reject;
  let received;
  const operation = new Promise((_, fail) => { reject = fail; });
  const ui = fixture({ saved: (...args) => { received = args; return operation; } });
  ui.openPopoverForSection(ui.sections.value[8]);
  ui.retain();
  const save = ui.saveAndOpenReader();
  assert.equal(ui.isSaving.value, true);
  assert.deepEqual(Array.from(received[2]), ['fingerprint']);
  assert.deepEqual(ui.events, []);
  reject(Error('disk full'));
  await save;
  assert.match(ui.failure.value, /保存失败.*disk full/);
  assert.deepEqual(ui.events, []);
  assert.equal(ui.isSaving.value, false);
  assert.equal(ui.library.activeAuditNovel.value.book.id, 'book');
});

test('store publishes sections only after native commit and propagates failures', async () => {
  const loader = require("./support/loader.cjs");
  let complete;
  let calls = 0;
  const pending = new Promise(resolve => { complete = resolve; });
  const store = loader({ invoke: async () => { calls++; return pending; } })("stores/library.ts");
  store.books.value = [{ id: 'book', chapters: 1, sections: [{ title: 'Original' }], currentChapterIndex: 0 }];
  const saving = store.saveAuditedSections('book', [{ title: 'Replacement', chunks: [], body: 'Text' }], ['key']);
  await Promise.resolve();
  assert.equal(store.books.value[0].sections[0].title, 'Original');
  complete({ anomalies: [] });
  await saving;
  assert.equal(calls, 1);
  assert.equal(store.books.value[0].sections[0].title, 'Replacement');

  const broken = loader({ invoke: async () => { throw Error("database unavailable"); } })("stores/library.ts");
  broken.books.value = [{ id: 'book', chapters: 1, sections: [{ title: 'Original' }] }];
  await assert.rejects(broken.saveAuditedSections('book', [], []), /database unavailable/);
  assert.equal(broken.books.value[0].sections[0].title, 'Original');
});

test('saving or skipping opens the audited book even when another book was active', async () => {
  for (const action of ['saveAndOpenReader', 'skipAndOpenReader']) {
    const ui = fixture();
    ui.library.activeBook.value = { id: 'other', title: 'Another book' };
    await ui[action]();
    assert.deepEqual(ui.selected, ['book']);
    assert.deepEqual(ui.events, ['open-reader']);
  }
});
