import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { requireWasm } from "./require-wasm.mjs";

// Regenerates (or, with --check, verifies) every native fixture the tests and
// the gallery pin, in dependency order: results writes src/fixtures/results/
// bond.json, which the scenario application, horizon return, templates, attribution,
// cashflows and details embed (attribution also reads the scenario
// application cases); curves writes the curve-link market that market-browser
// extends, and market-browser's snapshot is what curve-query cuts down; covenants reads the analyst result that statements writes;
// gen-gallery copies the fixtures into docs-site last. Each generator runs in its own process so the
// WASM instances do not share memory.
const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const checking = process.argv.includes("--check");
requireWasm(["pkg-node"]);
const generators = [
  "tests/valuations/results/generate.mjs",
  "tests/portfolio/generate.mjs",
  "tests/portfolio/margin/generate.mjs",
  "tests/scenarios/application/generate.mjs",
  "tests/scenarios/horizon/generate.mjs",
  "tests/scenarios/templates/generate.mjs",
  "tests/attribution/generate.mjs",
  "tests/valuations/cashflows/generate.mjs",
  "tests/valuations/details/generate.mjs",
  "tests/valuations/loan-yield/generate.mjs",
  "tests/valuations/bond-credit/generate.mjs",
  "tests/valuations/convertible/generate.mjs",
  "tests/valuations/workout-ladder/generate.mjs",
  "tests/valuations/quote-converter/generate.mjs",
  "tests/core/curves/generate.mjs",
  "tests/core/market-browser/generate.mjs",
  "tests/core/curve-query/generate.mjs",
  "tests/core/cubes/generate.mjs",
  "tests/calibration/generate.mjs",
  "tests/valuations/scenario-table/generate.mjs",
  "tests/statements/generate.mjs",
  "tests/statements/wacc/generate.mjs",
  "tests/statements/normalization/generate.mjs",
  "tests/statements/capital-structure/generate.mjs",
  "tests/statements/relative-value/generate.mjs",
  "tests/statements/export/generate.mjs",
  "tests/covenants/generate.mjs",
  "tests/analytics/generate.mjs",
  "tests/models/credit/generate.mjs",
  "tests/models/risk/generate.mjs",
  "tests/models/options/generate.mjs",
  "tests/models/liability-management/generate.mjs",
  "tests/models/recovery/generate.mjs",
  "tests/margin/generate.mjs",
  "scripts/gen-gallery.mjs",
];
for (const script of generators) {
  console.log(`${checking ? "check" : "generate"} ${script}`);
  try {
    execFileSync(process.execPath, [script, ...(checking ? ["--check"] : [])], {
      cwd: packageRoot,
      stdio: "inherit",
    });
  } catch (error) {
    console.error(`${script} failed (exit ${error.status ?? "?"}).`);
    process.exit(error.status ?? 1);
  }
}
