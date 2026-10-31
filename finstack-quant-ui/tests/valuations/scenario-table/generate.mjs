import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const instrumentSource =
    "finstack-quant/valuations/tests/instruments/json_examples/structured_credit.json",
  marketSource =
    "finstack-quant-ui/tests/valuations/instruments/pricing-market.json";
const instrumentJson = await readFile(
    new URL(`../../../../${instrumentSource}`, import.meta.url),
    "utf8",
  ),
  marketJson = await readFile(
    new URL(`../../../../${marketSource}`, import.meta.url),
    "utf8",
  );
const instrument = JSON.parse(instrumentJson),
  factor = structuredClone(instrument);
factor.instrument.spec.tranches.tranches[0].current_balance.amount = "50000000";
factor.instrument.spec.pool.assets[0].balance.amount = "50000000";
const grid = {
    cprs: [0, 0.05, 0.15],
    cdrs: [0, 0.03],
    severities: [0.2, 0.6],
    recovery_lag: null,
  },
  asOf = instrument.instrument.spec.closing_date,
  trancheId = "CLONOTES-A";
const cases = [instrument, factor].map((input, index) => {
  const instrumentJson = native.validateInstrumentJson(JSON.stringify(input));
  const deal = native.StructuredCredit.fromJson(instrumentJson);
  try {
    const table = native.trancheScenarioTable(
      deal,
      trancheId,
      marketJson,
      asOf,
      JSON.stringify(grid),
    );
    return {
      name: index ? "factor-adjusted" : "original",
      instrument: JSON.parse(instrumentJson),
      table,
      units: native.scenarioTableFieldUnits(table.currency),
    };
  } finally {
    deal.free();
  }
});
await emit(
  new URL("./cases.json", import.meta.url),
  JSON.stringify({
    instrumentSource,
    instrumentSha256: createHash("sha256").update(instrumentJson).digest("hex"),
    marketSource,
    marketSha256: createHash("sha256").update(marketJson).digest("hex"),
    asOf,
    trancheId,
    grid,
    cases,
  }),
);
if (!checking)
  console.log(
    cases.map((c) => ({
      name: c.name,
      prices: c.table.cells.map((cell) => cell.price),
    })),
  );
// Tranche analytics: the fixed-rate CLONOTES-A above (discount margin fails
// natively) and the floating SENIOR note of the CLO mezzanine regression golden,
// whose market is its calibration envelope's final_market.
const goldenSource =
  "finstack-quant/valuations/tests/golden/data/pricing/regression_goldens/structured_credit/clo_mezzanine_base_case.json";
const goldenText = await readFile(
  new URL(`../../../../${goldenSource}`, import.meta.url),
  "utf8",
);
const golden = JSON.parse(goldenText);
const calibrated = native.calibrate(JSON.stringify(golden.market.envelope))
  .result.final_market;
/** Native exception as the worker transfers it: name, kind and message. */
function failure(call) {
  try {
    return { value: call() };
  } catch (error) {
    return {
      error: { name: error.name, kind: error.kind, message: error.message },
    };
  }
}
const trancheMetricIds = ["dv01", "default01", "prepayment01", "severity01"];
const trancheCases = [
  {
    name: "fixed",
    source: instrumentSource,
    sha256: createHash("sha256").update(instrumentJson).digest("hex"),
    instrument,
    market: JSON.parse(marketJson),
    asOf,
    trancheId,
    marketPricePct: 100,
  },
  {
    name: "floating",
    source: goldenSource,
    sha256: createHash("sha256").update(goldenText).digest("hex"),
    instrument: golden.instrument,
    market: calibrated,
    asOf: golden.metadata.valuation_date,
    trancheId: "SENIOR",
    marketPricePct: 25,
  },
].map(({ instrument, market, ...entry }) => {
  const instrumentJson = native.validateInstrumentJson(
    JSON.stringify(instrument),
  );
  const marketText = JSON.stringify(market);
  const deal = native.StructuredCredit.fromJson(instrumentJson);
  const { trancheId, asOf, marketPricePct } = entry;
  try {
    return {
      ...entry,
      instrument: JSON.parse(instrumentJson),
      market,
      metrics: trancheMetricIds,
      results: {
        // No price and no metric ids: Rust's default target.
        defaultMetrics: failure(() =>
          native.trancheMetrics(deal, trancheId, marketText, asOf),
        ),
        metrics: failure(() =>
          native.trancheMetrics(
            deal,
            trancheId,
            marketText,
            asOf,
            marketPricePct,
            trancheMetricIds,
          ),
        ),
        metricUnits: failure(() => native.trancheMetricFieldUnits("USD")),
        solveUnits: failure(() => native.trancheSolveFieldUnits()),
        breakevenCdr: failure(() =>
          native.trancheBreakevenCdr(deal, trancheId, marketText, asOf),
        ),
        discountMargin: failure(() =>
          native.trancheDiscountMargin(
            deal,
            trancheId,
            marketText,
            asOf,
            marketPricePct,
          ),
        ),
        oas: failure(() =>
          native.trancheOas(deal, trancheId, marketText, asOf, marketPricePct),
        ),
        schedule: failure(() => {
          const cashflows = deal.trancheCashflows(trancheId, marketText, asOf);
          return {
            cashflows,
            table: native.trancheCashflowsTable(cashflows),
          };
        }),
      },
    };
  } finally {
    deal.free();
  }
});
await emit(
  new URL("./tranche-analytics.json", import.meta.url),
  JSON.stringify({ cases: trancheCases }),
);
if (!checking)
  console.log(
    trancheCases.map((c) => ({
      name: c.name,
      results: Object.fromEntries(
        Object.entries(c.results).map(([key, result]) => [
          key,
          result.error ? result.error.message : "ok",
        ]),
      ),
    })),
  );
// Deal entry: every family StructuredCredit.newStandard accepts, from the schema's
// Rust example params, with the tranches a selector lists and the proof it simulates
// on the UI market.
const paramsSource =
  "finstack-quant/valuations/schemas/common/1/standard_deal_params.schema.json";
const params = JSON.parse(
  await readFile(
    new URL(`../../../../${paramsSource}`, import.meta.url),
    "utf8",
  ),
).examples[0];
const dealGrid = {
  cprs: [0, 0.1],
  cdrs: [0, 0.05],
  severities: [0.4],
  recovery_lag: null,
};
const dealCases = ["abs", "clo", "cmbs", "rmbs"].map((family) => {
  const deal = native.StructuredCredit.newStandard(
    family,
    JSON.stringify(params),
  );
  try {
    const instrumentJson = deal.toJson();
    const tranches = deal.tranches;
    return {
      family,
      instrumentJson,
      tranches,
      table: native.trancheScenarioTable(
        deal,
        tranches.tranches[0].id,
        marketJson,
        params.closing_date,
        JSON.stringify(dealGrid),
      ),
    };
  } finally {
    deal.free();
  }
});
await emit(
  new URL("./standard-deals.json", import.meta.url),
  JSON.stringify({
    paramsSource,
    params,
    marketSource,
    marketSha256: createHash("sha256").update(marketJson).digest("hex"),
    asOf: params.closing_date,
    grid: dealGrid,
    cases: dealCases,
  }),
);
// Pool report: a pool with real obligors, industries and ratings (the CLO mezzanine
// golden), the example CLO, whose one asset carries no attributes, and an empty pool.
const emptyPool = structuredClone(golden.instrument);
emptyPool.instrument.spec.pool.assets = [];
const poolCases = [
  {
    name: "golden",
    instrument: golden.instrument,
    asOf: golden.metadata.valuation_date,
  },
  { name: "unattributed", instrument, asOf },
  {
    name: "empty",
    instrument: emptyPool,
    asOf: golden.metadata.valuation_date,
  },
].map((entry) => {
  const instrumentJson = native.validateInstrumentJson(
    JSON.stringify(entry.instrument),
  );
  const deal = native.StructuredCredit.fromJson(instrumentJson);
  try {
    const report = native.poolReport(deal.pool, entry.asOf);
    return {
      name: entry.name,
      instrument: JSON.parse(instrumentJson),
      asOf: entry.asOf,
      report,
      units: native.poolReportFieldUnits(report.total_balance.currency),
    };
  } finally {
    deal.free();
  }
});
await emit(
  new URL("./pool-report.json", import.meta.url),
  JSON.stringify({ goldenSource, instrumentSource, cases: poolCases }),
);
