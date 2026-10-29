// Captures a native covenant compliance series over the analyst statement
// model for the presentation-only covenant-compliance gallery example. Runs
// after tests/statements/generate.mjs, whose analyst-result.json it reads.
// Rerun with `mise run ui-fixtures` after rebuilding the WASM package.
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { serializeHost } from "../../src/codec.mjs";
import { emit } from "../../scripts/fixture-output.mjs";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const fixtures = new URL("../../src/fixtures/", import.meta.url);
const read = (name) => readFile(new URL(name, fixtures), "utf8");

const modelJson = await read("statements/analyst-model.json");
const resultJson = await read("statements/analyst-result.json");
const engineJson = await read("covenants/analyst-covenants.json");

// Rust owns the whole period and non-finite policy
// (statements_analytics.evaluateCovenants): every result period is tested on
// its last day, with the covenants applicable on that date, and a metric is
// read only when an applicable covenant needs it. The analyst `leverage` is
// NaN for 2024Q1-Q3 (LTM EBITDA needs four quarters), so analyst-covenants.json
// opens the window that adds max_debt_to_ebitda on 2024-10-01; the earlier
// dates test min_interest_coverage only. A window replaces the top-level specs
// on its dates, so it repeats min_interest_coverage.
const engine = native.CovenantEngine.fromJson(engineJson);
let series;
try {
  series = native.evaluateCovenants(resultJson, engine, modelJson);
} finally {
  engine.free();
}
await emit(
  new URL("covenants/analyst-covenant-series.json", fixtures),
  serializeHost(series),
);
