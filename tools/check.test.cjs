/** Challenge registry rejection and local process receipts with explicit failures. */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const { validate, run } = require("./check.cjs");
const original = require("../development.json");

test("invalid registry versions, fields, duplicate identities, levels, paths and budgets are rejected", () => {
  assert.equal(validate(structuredClone(original)).version, 1);
  for (const mutate of [config => { config.version = 2; }, config => { config.extra = true; },
    config => { config.components[0].level = null; }, config => { config.checks.push(config.checks[0]); },
    config => { config.checks[0].cwd = ".."; }, config => { config.checks[0].timeoutMs = 0; }]) {
    const config = structuredClone(original);
    mutate(config);
    assert.throws(() => validate(config));
  }
});

test("receipts distinguish successful, rejected and timed-out children", async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "saga-check-"));
  try {
    const cases = [["success", "console.log('observed')", 5000, true], ["failure", "process.exit(7)", 5000, false],
      ["timeout", "setInterval(() => {}, 1000)", 100, false]];
    for (const [id, script, timeoutMs, expected] of cases) {
      const receipt = await run({ id, program: "node", cwd: ".", args: ["-e", script], timeoutMs }, path.join(directory, id + ".log"));
      assert.equal(receipt.passed, expected);
      if (id === "failure") assert.equal(receipt.exitCode, 7);
      if (id === "timeout") assert.equal(receipt.termination, "timeout");
      assert.equal(receipt.sha256.length, 64);
    }
  } finally {
    for (const entry of fs.readdirSync(directory)) fs.unlinkSync(path.join(directory, entry));
    fs.rmdirSync(directory);
  }
});
