import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

// Regenerates (or, with --check, verifies) every native fixture the tests and
// the gallery pin, in dependency order: results writes src/fixtures/results/
// bond.json, which cashflows and details embed; curves writes the curve-link
// market that market-browser extends; gen-gallery copies the fixtures into
// docs-site last. Each generator runs in its own process so the WASM instances
// do not share memory.
const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const checking = process.argv.includes("--check");
const nodePackage = new URL(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  import.meta.url,
);
if (!existsSync(nodePackage)) {
  console.error(
    "finstack-quant-wasm/pkg-node is not built; run `mise run wasm-build-node` (dev) or `mise run wasm-pkg` (release) first.",
  );
  process.exit(1);
}
const generators = [
  "tests/valuations/results/generate.mjs",
  "tests/valuations/cashflows/generate.mjs",
  "tests/valuations/details/generate.mjs",
  "tests/core/curves/generate.mjs",
  "tests/core/market-browser/generate.mjs",
  "tests/core/cubes/generate.mjs",
  "tests/calibration/generate.mjs",
  "tests/valuations/scenario-table/generate.mjs",
  "tests/statements/generate.mjs",
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
