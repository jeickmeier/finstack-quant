import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
import { serializeHost } from "../../../src/codec.mjs";
// The priced bond of the results fixture is the base, as for the scenario application.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondInput = await readRepoSource(bondPath);
const base = bondInput.value.request;
// ScenarioSpecs as the workbench builder writes them; omitted options are left to Rust's defaults.
const usdOis50 = {
  kind: "curve_parallel_bp",
  curve_kind: "discount",
  curve_id: "USD-OIS",
  bp: 50,
};
const roll3m = {
  id: "roll-3m-50bp",
  operations: [{ kind: "time_roll_forward", period: "3M" }, usdOis50],
};
const shift = { id: "usd-ois-50bp", operations: [usdOis50] };
// Requests: method and calendarId are the worker request fields; null stays unset.
const requests = [
  { name: "roll-3m-50bp", spec: roll3m, method: null, calendarId: null },
  { name: "usd-ois-50bp", spec: shift, method: null, calendarId: null },
  {
    name: "roll-3m-50bp-taylor",
    spec: roll3m,
    method: { taylor: {} },
    calendarId: "nyse",
  },
  // The bond market's raw hazard curve has no calibration recipe, so the
  // metrics-based method drops its quote-space credit metrics with a note.
  {
    name: "roll-3m-50bp-metrics-based",
    spec: roll3m,
    method: "metrics_based",
    calendarId: null,
  },
];
// The worker's ScenariosApi.validateScenarioSpec.
const canonical = (spec) =>
  JSON.stringify(native.parseScenarioSpec(JSON.stringify(spec)));
// The worker's ScenariosApi.horizonReturn.
const cases = requests.map(({ name, spec, method, calendarId }) => {
  const scenarioJson = canonical(spec);
  try {
    const report = native.computeHorizonReturn(
      base.instrumentJson,
      base.marketJson,
      base.asOf,
      scenarioJson,
      method ?? undefined,
      undefined,
      calendarId ?? undefined,
    );
    return {
      name,
      spec,
      scenarioJson,
      method,
      calendarId,
      report,
      explain: native.horizonResultExplainText(report),
      attributionBuckets: native.pnlAttributionPctOfTotalByBucket(
        report.attribution,
      ),
    };
  } catch (error) {
    return {
      name,
      spec,
      scenarioJson,
      method,
      calendarId,
      error: { kind: error.kind, message: error.message },
    };
  }
});
await emit(
  new URL("cases.json", import.meta.url),
  serializeHost({
    description:
      "Native horizon total returns of the results-fixture bond: the Rust-built spec, scenarios.computeHorizonReturn (omitted method, config and calendar left to Rust) horizonResultExplainText and pnlAttributionPctOfTotalByBucket, or the native error.",
    sources: [bondInput.source],
    base: { asOf: base.asOf },
    cases,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native horizon returns.`,
);
