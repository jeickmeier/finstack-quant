import { checking, emit } from "../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../scripts/fixture-inputs.mjs";
import { serializeHost } from "../../src/codec.mjs";
// Convertible credit widening 150bp -> 300bp over one day (attribution crate fixture).
const sourcePath =
  "finstack-quant/attribution/tests/fixtures/production_convertible_credit.json";
const sourceInput = await readRepoSource(sourcePath);
const source = sourceInput.value;
const methods = [
  { name: "parallel", method: "parallel" },
  // Rust's default factor order; the waterfall steps chain in it.
  { name: "waterfall", method: { waterfall: native.defaultWaterfallOrder() } },
  { name: "metrics_based", method: "metrics_based" },
  { name: "taylor", method: { taylor: {} } },
];
const cases = methods.map(({ name, method }) => {
  const [attribution] = native.attributePnlMany(
    {
      as_of_t0: source.as_of_t0,
      as_of_t1: source.as_of_t1,
      market_t0: source.market_t0,
      market_t1: source.market_t1,
      method,
    },
    [source.instrument],
  );
  return {
    name,
    method,
    attribution,
    // The Rust helpers a host passes to the report's optional props.
    explain: native.pnlAttributionExplainText(attribution),
    residualWithinTolerance:
      native.pnlAttributionResidualWithinTolerance(attribution),
    buckets: native.pnlAttributionPctOfTotalByBucket(attribution),
  };
});
// The workbench's P&L explain: the results-fixture bond attributed by the
// worker's AttributionApi from its market and date (the pinned T0) to the
// market a SCN-1 scenario returns (T1), canonical market texts spliced.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondInput = await readRepoSource(bondPath);
const bond = bondInput.value.request;
const scenariosPath =
  "finstack-quant-ui/tests/scenarios/application/cases.json";
const scenariosInput = await readRepoSource(scenariosPath);
const scenarios = scenariosInput.value.cases;
function canonicalMarket(json) {
  const market = native.MarketContext.fromJson(json);
  try {
    return market.toJson();
  } finally {
    market.free();
  }
}
function attributed(inputs) {
  try {
    const [attribution] = native.attributePnlMany(inputs, [
      bond.instrumentJson,
    ]);
    return {
      ok: true,
      attribution,
      explain: native.pnlAttributionExplainText(attribution),
      residualWithinTolerance:
        native.pnlAttributionResidualWithinTolerance(attribution),
      buckets: native.pnlAttributionPctOfTotalByBucket(attribution),
    };
  } catch (error) {
    return { ok: false, kind: error.kind, message: error.message };
  }
}
const marketT0 = canonicalMarket(bond.marketJson);
const workbench = [
  { scenario: "usd-ois-50bp", name: "parallel", method: "parallel" },
  {
    scenario: "usd-ois-50bp",
    name: "waterfall",
    method: { waterfall: native.defaultWaterfallOrder() },
  },
  // HZ has no calibration recipe: Rust drops the quote-space credit metrics with a note.
  { scenario: "usd-ois-50bp", name: "metrics_based", method: "metrics_based" },
  {
    scenario: "roll-1m-25bp",
    name: "waterfall",
    method: { waterfall: native.defaultWaterfallOrder() },
  },
].map(({ scenario, name, method }) => {
  const { scenarioJson } = scenarios.find((entry) => entry.name === scenario);
  const applied = native.applyScenario(
    scenarioJson,
    marketT0,
    bond.asOf,
    undefined,
    undefined,
  );
  const asOfT1 = native.applicationReportEffectiveAsOf(
    applied.report,
    bond.asOf,
  );
  const marketT1 = canonicalMarket(JSON.stringify(applied.market));
  return {
    scenario,
    name,
    method,
    asOfT0: bond.asOf,
    asOfT1,
    ...attributed(
      `{"as_of_t0":${JSON.stringify(bond.asOf)},"as_of_t1":${JSON.stringify(asOfT1)},"market_t0":${marketT0},"market_t1":${marketT1},"method":${JSON.stringify(method)}}`,
    ),
  };
});
await emit(
  new URL("cases.json", import.meta.url),
  serializeHost({
    description:
      "Native single-instrument P&L attribution of a convertible bond whose credit spreads widen from 150bp to 300bp over one day, under the four Rust methods (attributePnlMany), with Rust's explain text, residual check and share of total per factor. `workbench` attributes the results-fixture bond from its market and date to the market of a market-scenario case (the workbench P&L explain); the markets themselves are not stored.",
    sources: [sourceInput.source, bondInput.source, scenariosInput.source],
    cases,
    workbench,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length + workbench.length} native P&L attributions.`,
);
