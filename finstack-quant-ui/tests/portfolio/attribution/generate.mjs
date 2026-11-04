import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
import { serializeHost } from "../../../src/codec.mjs";
// The workbench's margined rates book over the pricing market on the margin
// fixture's date (T0). The host's closing markets (T1) are those two
// market-scenario cases return from T0: a 50bp USD-OIS shock on the same date,
// and a one-month roll with a 25bp shock, which takes the swaps past a SOFR
// fixing the pricing market has no FIXING: series for.
const workbench = await readRepoSource(
  "finstack-quant-ui/tests/portfolio/workbench/cases.json",
);
const market = await readRepoSource(
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
);
const scenarios = await readRepoSource(
  "finstack-quant-ui/tests/scenarios/application/cases.json",
);
const { specJson } = workbench.value;
const asOfT0 = workbench.value.request.asOf;
// The host's reconciliation tolerance: one cent of the USD book.
const reconciliationTolerance = 0.01;
function canonicalMarket(json) {
  const context = native.MarketContext.fromJson(json);
  try {
    return context.toJson();
  } finally {
    context.free();
  }
}
const marketT0Json = canonicalMarket(JSON.stringify(market.value));
// Each closing market as applyScenario returns it from T0, with Rust's effective date.
const closing = Object.fromEntries(
  ["usd-ois-50bp", "roll-1m-25bp"].map((name) => {
    const { scenarioJson } = scenarios.value.cases.find(
      (entry) => entry.name === name,
    );
    const applied = native.applyScenario(
      scenarioJson,
      marketT0Json,
      asOfT0,
      undefined,
      undefined,
    );
    return [
      name,
      {
        marketT1Json: canonicalMarket(JSON.stringify(applied.market)),
        asOfT1: native.applicationReportEffectiveAsOf(applied.report, asOfT0),
      },
    ];
  }),
);
const methods = [
  ["parallel", "parallel"],
  // Rust's default factor order, as the panel sends it.
  ["waterfall", { waterfall: native.defaultWaterfallOrder() }],
  ["metrics_based", "metrics_based"],
  ["taylor", { taylor: {} }],
];
const book = native.Portfolio.fromSpec(specJson);
const t0 = native.MarketContext.fromJson(marketT0Json);
// The worker's attributePortfolioPnl over the raw package: one native
// attribution, then Rust's explain text, reconciliation and buckets for the
// book and for each by_position entry.
function attributed(t1, asOfT1, method, tolerance = reconciliationTolerance) {
  try {
    const attribution = native.attributePortfolioPnl(
      book,
      t0,
      t1,
      asOfT0,
      asOfT1,
      method,
    );
    return {
      ok: true,
      report: {
        attribution,
        explain: native.portfolioAttributionExplainText(attribution),
        reconciliation: native.portfolioAttributionReconciliationCheck(
          attribution,
          tolerance,
        ),
        buckets: native.portfolioAttributionPctOfTotalByBucket(attribution),
        positions: Object.entries(attribution.by_position).map(
          ([positionId, position]) => ({
            positionId,
            attribution: position,
            explain: native.pnlAttributionExplainText(position),
            residualWithinTolerance:
              native.pnlAttributionResidualWithinTolerance(position),
            buckets: native.pnlAttributionPctOfTotalByBucket(position),
          }),
        ),
      },
    };
  } catch (error) {
    return { ok: false, kind: error.kind, message: error.message };
  }
}
// A host demanding exact reconciliation (tolerance 0): Rust's twelve buckets
// include the residual, so the book reconciles by construction and only the
// floating-point remainder of their sum (about 5e-13 USD under metrics_based)
// is left; at zero tolerance Rust says it does not reconcile. Positions under
// metrics_based keep residuals outside Rust's own tolerance either way.
const exactTolerance = 0;
let cases;
let exact;
try {
  cases = Object.entries(closing).flatMap(([scenario, point]) => {
    const t1 = native.MarketContext.fromJson(point.marketT1Json);
    try {
      return methods.map(([name, method]) => ({
        scenario,
        name,
        method,
        ...attributed(t1, point.asOfT1, method),
      }));
    } finally {
      t1.free();
    }
  });
  const point = closing["usd-ois-50bp"];
  const t1 = native.MarketContext.fromJson(point.marketT1Json);
  try {
    exact = {
      scenario: "usd-ois-50bp",
      name: "metrics_based",
      method: "metrics_based",
      reconciliationTolerance: exactTolerance,
      ...attributed(t1, point.asOfT1, "metrics_based", exactTolerance),
    };
  } finally {
    t1.free();
  }
} finally {
  book.free();
  t0.free();
}
await emit(
  new URL("cases.json", import.meta.url),
  serializeHost({
    description:
      "Native portfolio P&L attribution of the workbench's margined rates book (two bilateral swaps and a deposit) from the pricing market on the margin fixture's date to two host closing markets, under every Rust method: portfolio.attributePortfolioPnl, then portfolioAttributionExplainText, portfolioAttributionReconciliationCheck at a one-cent tolerance and portfolioAttributionPctOfTotalByBucket, and per position attribution.pnlAttributionExplainText, pnlAttributionResidualWithinTolerance and pnlAttributionPctOfTotalByBucket. The usd-ois-50bp closing market keeps the date; the roll-1m-25bp one rolls a month, where every method returns Rust's missing-fixing error. exact repeats metrics_based on usd-ois-50bp at a host tolerance of zero, where Rust reports the book not reconciled. The closing markets are stored because a host supplies them.",
    sources: [workbench.source, market.source, scenarios.source],
    specJson,
    asOfT0,
    reconciliationTolerance,
    closing,
    cases,
    exact,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native portfolio attributions.`,
);
