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
