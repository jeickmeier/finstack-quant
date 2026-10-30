import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { checking, emit } from "../../scripts/fixture-output.mjs";
import { serializeHost } from "../../src/codec.mjs";
const repository = new URL("../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// Convertible credit widening 150bp -> 300bp over one day (attribution crate fixture).
const sourcePath =
  "finstack-quant/attribution/tests/fixtures/production_convertible_credit.json";
const sourceBytes = await readFile(new URL(sourcePath, repository));
const source = JSON.parse(sourceBytes);
// The PnlAttribution buckets pnl-attribution-report lists.
const fields = [
  "carry",
  "rates_curves_pnl",
  "credit_curves_pnl",
  "inflation_curves_pnl",
  "correlations_pnl",
  "fx_pnl",
  "fx_translation_pnl",
  "vol_pnl",
  "cross_factor_pnl",
  "model_params_pnl",
  "market_scalars_pnl",
  "residual",
];
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
    pctOfTotal: Object.fromEntries(
      fields.map((field) => [
        field,
        native.pnlAttributionPctOfTotal(
          attribution,
          Number(attribution[field].amount),
        ) ?? null,
      ]),
    ),
  };
});
// The workbench's P&L explain: the results-fixture bond attributed by the
// worker's AttributionApi from its market and date (the pinned T0) to the
// market a SCN-1 scenario returns (T1), canonical market texts spliced.
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const bondBytes = await readFile(new URL(bondPath, repository));
const bond = JSON.parse(bondBytes).request;
const scenariosPath =
  "finstack-quant-ui/tests/scenarios/application/cases.json";
const scenariosBytes = await readFile(new URL(scenariosPath, repository));
const scenarios = JSON.parse(scenariosBytes).cases;
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
      pctOfTotal: Object.fromEntries(
        fields.map((field) => [
          field,
          native.pnlAttributionPctOfTotal(
            attribution,
            Number(attribution[field].amount),
          ) ?? null,
        ]),
      ),
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
  // Fails on the workbench market: cs01 needs a calibration recipe for HZ.
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
  const asOfT1 = applied.report.time_roll?.new_date ?? bond.asOf;
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
    sources: [
      {
        path: sourcePath,
        sha256: createHash("sha256").update(sourceBytes).digest("hex"),
      },
      {
        path: bondPath,
        sha256: createHash("sha256").update(bondBytes).digest("hex"),
      },
      {
        path: scenariosPath,
        sha256: createHash("sha256").update(scenariosBytes).digest("hex"),
      },
    ],
    cases,
    workbench,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length + workbench.length} native P&L attributions.`,
);
