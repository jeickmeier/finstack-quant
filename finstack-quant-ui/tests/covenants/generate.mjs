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

// Row building (no Rust statement-series evaluation is bound; plan RT-23 was
// declined). This is the whole policy:
// 1. Each metric id the engine reads is exported with
//    statements.nodeToDatedSchedule (default "end" convention: the period's
//    last inclusive day), so every schedule has one entry per model period in
//    timeline order and the rows zip by index.
// 2. Test dates start at the first period listed below (2024Q3).
// 3. A non-finite value drops that metric key from its row, never the row:
//    CovenantEngine#evaluateSeries rejects NaN at the boundary, and Rust's
//    window applicability decides whether the metric was required (a missing
//    required metric fails with not_found). The analyst `leverage` is NaN for
//    2024Q1-Q3 (LTM EBITDA needs four quarters), so analyst-covenants.json
//    opens the window that adds max_debt_to_ebitda on 2024-10-01. A window
//    replaces the top-level specs on its dates, so it repeats
//    min_interest_coverage.
const firstPeriod = "2024Q3";
const metricIds = ["leverage", "interest_coverage"];
const periods = JSON.parse(modelJson).periods.map((period) => period.id);
const start = periods.indexOf(firstPeriod);
if (start < 0) throw new Error(`Model has no period ${firstPeriod}`);
const schedules = metricIds.map((id) =>
  native.nodeToDatedSchedule(modelJson, resultJson, id),
);
const rows = schedules[0].slice(start).map(([date], offset) => ({
  date,
  metrics: Object.fromEntries(
    metricIds
      .map((id, index) => [id, schedules[index][start + offset][1]])
      .filter(([, value]) => Number.isFinite(value)),
  ),
}));

const engine = native.CovenantEngine.fromJson(engineJson);
let series;
try {
  series = engine.evaluateSeries(rows);
} finally {
  engine.free();
}
await emit(
  new URL("covenants/analyst-covenant-series.json", fixtures),
  serializeHost(series),
);
