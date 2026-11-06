import { spawn } from "node:child_process";
import { availableParallelism } from "node:os";
import { fileURLToPath } from "node:url";
import { requireWasm } from "./require-wasm.mjs";

// Regenerates (or, with --check, verifies) every native fixture the tests and
// the gallery pin. Each generator runs in its own process, so the WASM
// instances do not share memory, and generators run concurrently once the
// generators whose output they read have finished. Under --check the order
// does not matter (every generator reads committed files), but write mode
// needs it, so each entry lists every generator whose fixture it reads.
// gen-gallery copies the fixtures into docs-site last.
const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const checking = process.argv.includes("--check");
requireWasm(["pkg-node"]);

const results = "tests/valuations/results/generate.mjs";
const portfolioMargin = "tests/portfolio/margin/generate.mjs";
const workbench = "tests/portfolio/workbench/generate.mjs";
const application = "tests/scenarios/application/generate.mjs";
const curves = "tests/core/curves/generate.mjs";
const marketBrowser = "tests/core/market-browser/generate.mjs";
const calibration = "tests/calibration/generate.mjs";
const statements = "tests/statements/generate.mjs";
const generators = new Map([
  [results, []],
  ["tests/portfolio/generate.mjs", []],
  [portfolioMargin, []],
  [workbench, [portfolioMargin]],
  ["tests/portfolio/exposures/generate.mjs", [workbench]],
  [application, [results]],
  ["tests/portfolio/attribution/generate.mjs", [workbench, application]],
  ["tests/portfolio/returns/generate.mjs", []],
  ["tests/portfolio/brinson/generate.mjs", []],
  ["tests/scenarios/horizon/generate.mjs", [results]],
  ["tests/scenarios/templates/generate.mjs", [results]],
  ["tests/attribution/generate.mjs", [results, application]],
  ["tests/valuations/cashflows/generate.mjs", [results]],
  ["tests/valuations/details/generate.mjs", [results]],
  ["tests/valuations/loan-yield/generate.mjs", []],
  ["tests/valuations/bond-credit/generate.mjs", []],
  ["tests/valuations/convertible/generate.mjs", []],
  ["tests/valuations/workout-ladder/generate.mjs", []],
  ["tests/valuations/quote-converter/generate.mjs", []],
  [curves, []],
  [calibration, []],
  [marketBrowser, [curves, calibration]],
  ["tests/core/curve-query/generate.mjs", [marketBrowser]],
  ["tests/core/cubes/generate.mjs", []],
  ["tests/valuations/scenario-table/generate.mjs", []],
  [statements, []],
  ["tests/statements/wacc/generate.mjs", []],
  ["tests/statements/normalization/generate.mjs", [statements]],
  ["tests/statements/capital-structure/generate.mjs", []],
  ["tests/statements/relative-value/generate.mjs", []],
  ["tests/statements/export/generate.mjs", [statements]],
  ["tests/covenants/generate.mjs", [statements]],
  ["tests/analytics/generate.mjs", []],
  ["tests/analytics/correlation/generate.mjs", []],
  ["tests/models/credit/generate.mjs", []],
  ["tests/models/risk/generate.mjs", []],
  ["tests/models/options/generate.mjs", []],
  ["tests/models/liability-management/generate.mjs", []],
  ["tests/models/recovery/generate.mjs", []],
  ["tests/margin/generate.mjs", []],
]);
generators.set("scripts/gen-gallery.mjs", [...generators.keys()]);

/** Run one generator, then print its buffered output so logs do not interleave. */
function run(script) {
  return new Promise((resolve) => {
    const output = [];
    const child = spawn(
      process.execPath,
      [script, ...(checking ? ["--check"] : [])],
      { cwd: packageRoot, stdio: ["ignore", "pipe", "pipe"] },
    );
    child.stdout.on("data", (chunk) => output.push(chunk));
    child.stderr.on("data", (chunk) => output.push(chunk));
    const finish = (status, reason) => {
      process.stdout.write(
        `${checking ? "check" : "generate"} ${script}\n${Buffer.concat(output)}`,
      );
      if (status !== 0) console.error(`${script} failed (${reason}).`);
      resolve(status === 0);
    };
    child.once("error", (error) => finish(1, error.message));
    child.once("close", (code, signal) =>
      finish(code ?? 1, signal ? `signal ${signal}` : `exit ${code}`),
    );
  });
}

const pending = new Map(generators);
const finished = new Set();
const failed = [];
let running = 0;
const slots = availableParallelism();
await new Promise((resolve) => {
  const schedule = () => {
    // After a failure, start nothing new and let running generators finish.
    if (!failed.length)
      for (const [script, after] of pending) {
        if (running === slots) break;
        if (!after.every((dependency) => finished.has(dependency))) continue;
        pending.delete(script);
        running += 1;
        run(script).then((ok) => {
          running -= 1;
          if (ok) finished.add(script);
          else failed.push(script);
          schedule();
        });
      }
    if (running === 0) resolve();
  };
  schedule();
});
if (failed.length) process.exit(1);
