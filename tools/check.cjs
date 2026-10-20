/** Bounded local verification with source hashes and preserved process receipts; not a release gate. */
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawn, execFileSync } = require("node:child_process");

// Resolve every checked path from this tool, never from an accidental caller directory.
const root = path.resolve(__dirname, "..");

/** Reject unknown registry fields instead of silently interpreting future formats. */
function fields(value, expected) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).sort().join() !== [...expected].sort().join()) throw new Error("unexpected registry fields");
}

/** Resolve an existing workspace directory without permitting a parent escape. */
function directory(value) {
  if (typeof value !== "string" || path.isAbsolute(value)) throw new Error("directory must be repository relative");
  const resolved = fs.realpathSync(path.resolve(root, value));
  if (resolved !== root && !resolved.startsWith(root + path.sep)) throw new Error("directory escapes workspace");
  if (!fs.statSync(resolved).isDirectory()) throw new Error("expected a directory");
  return resolved;
}

/** Validate the limited feedback registry; formal obligations and release schemas are not delivered. */
function validate(config) {
  fields(config, ["version", "purpose", "qualification", "components", "environment", "checks"]);
  if (config.version !== 1 || config.purpose !== "local-feedback" || config.qualification !== "not-qualified") throw new Error("unsupported registry semantics");
  fields(config.environment, ["verifiedPlatform", "rust", "node", "typescript"]);
  if (Object.values(config.environment).some(value => typeof value !== "string" || !value)) throw new Error("invalid environment identity");
  for (const [entries, keys] of [[config.components, ["id", "path", "level"]], [config.checks, ["id", "program", "cwd", "args", "timeoutMs"]]]) {
    if (!Array.isArray(entries) || entries.length === 0) throw new Error("empty registry scope");
    const ids = new Set();
    for (const entry of entries) {
      fields(entry, keys);
      if (typeof entry.id !== "string" || !/^[a-z]+$/.test(entry.id) || ids.has(entry.id)) throw new Error("invalid or duplicate identity");
      ids.add(entry.id);
      if (keys.includes("level")) {
        if (!["L1", "L2", "L3"].includes(entry.level)) throw new Error("missing component level");
        directory(entry.path);
      } else {
        directory(entry.cwd);
        if (!["cargo", "node"].includes(entry.program) || !Array.isArray(entry.args) || !entry.args.length ||
            entry.args.some(argument => typeof argument !== "string") || !Number.isInteger(entry.timeoutMs) ||
            entry.timeoutMs < 1 || entry.timeoutMs > 180000) throw new Error("invalid process contract");
      }
    }
  }
  return config;
}

/** Hash maintained and untracked source while excluding the evidence being collected. */
function manifest() {
  const output = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: root, windowsHide: true }).toString();
  const files = [...new Set(output.split("\0").filter(name => name && !name.startsWith("docs/evidence/")))].sort();
  return files.filter(name => fs.existsSync(path.join(root, name))).map(name => ({
    path: name, sha256: crypto.createHash("sha256").update(fs.readFileSync(path.join(root, name))).digest("hex")
  }));
}

/** Run one owned child, retaining output and treating timeouts as failures. */
async function run(check, filename, toolchain) {
  if (check.program === "cargo" && !toolchain) throw new Error("explicit installed Rust toolchain required");
  const started = new Date().toISOString();
  const command = check.program === "node" ? process.execPath : "rustup";
  // rustup run without --install fails when the selected toolchain is absent.
  const args = check.program === "cargo" ? ["run", toolchain, "cargo", ...check.args] : check.args;
  return new Promise(resolve => {
    let termination = "exited";
    let bytes = 0;
    let error = null;
    fs.writeFileSync(filename, "", { flag: "wx" });
    const child = spawn(command, args, { cwd: directory(check.cwd), windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    function stop(reason) {
      if (termination !== "exited") return;
      termination = reason;
      if (!child.pid) return;
      if (process.platform === "win32") {
        try { execFileSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { windowsHide: true, stdio: "ignore" }); }
        catch (failure) { error = "process tree termination failed: " + failure.message; }
      } else child.kill("SIGKILL");
    }
    const timer = setTimeout(() => stop("timeout"), check.timeoutMs);
    function output(buffer) {
      bytes += buffer.length;
      if (bytes <= 8 * 1024 * 1024) fs.appendFileSync(filename, buffer);
      else stop("output-limit");
    }
    child.stdout.on("data", output);
    child.stderr.on("data", output);
    child.on("error", failure => { error = failure.message; });
    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      resolve({ ...check, command, arguments: args, started, finished: new Date().toISOString(), exitCode, signal, termination, error, bytes,
        passed: exitCode === 0 && termination === "exited" && error === null,
        log: path.basename(filename), sha256: crypto.createHash("sha256").update(fs.readFileSync(filename)).digest("hex") });
    });
  });
}

/** Validate without execution, or collect the explicitly requested local checks once. */
async function main() {
  const mode = process.argv[2];
  if (!["validate", "collect"].includes(mode) || process.argv.length !== 3) throw new Error("Usage: node tools/check.cjs validate|collect");
  const config = validate(JSON.parse(fs.readFileSync(path.join(root, "development.json"), "utf8")));
  if (mode === "validate") { console.log("Local feedback registry valid; no checks started."); return; }
  if (process.platform + "-" + process.arch !== config.environment.verifiedPlatform) throw new Error("collection platform has not been registered");
  const rust = execFileSync("rustup", ["run", config.environment.rust, "rustc", "--version"], { windowsHide: true }).toString().trim();
  const typescript = require("../apps/windows/node_modules/typescript/package.json").version;
  if (process.versions.node !== config.environment.node || !rust.startsWith("rustc " + config.environment.rust + " ") || typescript !== config.environment.typescript) {
    throw new Error("toolchain differs from registered verification environment");
  }
  const identity = new Date().toISOString().replace(/[:.]/g, "-") + "-" + crypto.randomUUID().slice(0, 8);
  const output = path.join(root, "docs/evidence", identity);
  fs.mkdirSync(output, { recursive: true });
  const before = manifest();
  const checks = [];
  for (const check of config.checks) {
    console.log("Checking " + check.id);
    const receipt = await run(check, path.join(output, check.id + ".log"), config.environment.rust);
    checks.push(receipt);
    fs.writeFileSync(path.join(output, "checks.json"), JSON.stringify(checks, null, 2) + "\n");
    console.log(check.id + ": " + (receipt.passed ? "passed" : "FAILED"));
  }
  const after = manifest();
  const unchanged = JSON.stringify(before) === JSON.stringify(after);
  const receipt = { identity, purpose: config.purpose, qualification: "not-qualified", environment: { platform: process.platform, architecture: process.arch, node: process.version, rust, typescript },
    before, after, unchanged, checks, passed: unchanged && checks.every(check => check.passed), trust: "local-unsealed" };
  fs.writeFileSync(path.join(output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
  console.log(path.relative(root, path.join(output, "receipt.json")));
  if (!receipt.passed) process.exitCode = 1;
}

module.exports = { validate, run };
if (require.main === module) main().catch(error => { console.error(error.message); process.exitCode = 1; });
