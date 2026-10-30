import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { checking, emit } from "../../scripts/fixture-output.mjs";
import { serializeHost } from "../../src/codec.mjs";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, import.meta.url), "utf8"));
// Gallery book: the bond and interest_rate_swap pricing cases (no patches),
// each held by its own entity, over the shared pricing market at 2023-12-28.
const pricing = await read("../valuations/instruments/pricing-cases.json");
const caseOf = (type) => pricing.cases.find((entry) => entry.type === type);
const bond = caseOf("bond");
const swap = caseOf("interest_rate_swap");
for (const entry of [bond, swap])
  if (entry.instrumentPatches.length || entry.marketPatches.length)
    throw new Error(`${entry.type} pricing case gained patches`);
const instrument = async (entry) =>
  (await read(`../../../${entry.instrumentSource.path}`)).instrument;
const [bondInstrument, swapInstrument] = await Promise.all([
  instrument(bond),
  instrument(swap),
]);
const gallerySpec = {
  id: "GALLERY-BOOK",
  name: "Rates and credit book",
  base_currency: "USD",
  as_of: bond.request.asOf,
  entities: {
    CREDIT: { id: "CREDIT", name: "Credit" },
    RATES: { id: "RATES", name: "Rates" },
  },
  positions: [
    {
      position_id: "BOND-1",
      entity_id: "CREDIT",
      instrument_id: bondInstrument.spec.id,
      instrument_spec: bondInstrument,
      quantity: 1,
      unit: "units",
    },
    {
      position_id: "IRS-1",
      entity_id: "RATES",
      instrument_id: swapInstrument.spec.id,
      instrument_spec: swapInstrument,
      quantity: 1,
      unit: "units",
    },
  ],
};
const pricingMarket = await read(
  "../valuations/instruments/pricing-market.json",
);
// Worker parity: the Py/WASM host-parity book (two USD deposits) at t0.
const { inputs, expected } = await read(
  "../../../finstack-quant/portfolio/tests/fixtures/host_parity.json",
);
// The worker's PortfolioApi over the raw package: valuePortfolio, then
// aggregateMetrics over the same market, then one base metric's series.
function report(specJson, marketJson, request, base) {
  const book = native.Portfolio.fromSpec(specJson);
  const market = native.MarketContext.fromJson(marketJson);
  try {
    const valuation = native.valuePortfolio(
      book,
      market,
      request.strictRisk ?? undefined,
      request.metrics ?? undefined,
    );
    const metrics = native.aggregateMetrics(valuation, market);
    const keys = Object.values(valuation.position_values).flatMap((position) =>
      Object.keys(position.valuation_result?.measures ?? {}),
    );
    return {
      request,
      spec: JSON.parse(book.toJson()),
      report: {
        positionIds: book.positionIds,
        entityIds: book.entityIds,
        valuation,
        metrics,
      },
      series: { base, entries: native.portfolioMetricsSeries(metrics, base) },
      metadata: native.metricMetadata([...new Set(keys)]),
    };
  } finally {
    book.free();
    market.free();
  }
}
const cases = [
  {
    file: "gallery.json",
    value: report(
      JSON.stringify(gallerySpec),
      JSON.stringify(pricingMarket),
      // A menu for a mixed book: cs01 is inapplicable to the swap and the
      // bond's cs01::<id> stays unaggregated.
      { metrics: ["dv01", "bucketed_dv01", "theta", "cs01"] },
      "bucketed_dv01",
    ),
  },
  {
    file: "host-parity.json",
    value: {
      ...report(
        JSON.stringify(inputs.portfolio_spec),
        JSON.stringify(inputs.market_t0),
        { metrics: ["dv01", "bucketed_dv01"] },
        "bucketed_dv01",
      ),
      // Values pinned by the Py/WASM host-parity tests.
      expected: {
        valuation_position_value_base: expected.valuation_position_value_base,
        valuation_entity_value: expected.valuation_entity_value,
        metrics_total_dv01: expected.metrics_total_dv01,
        metrics_position_dv01: expected.metrics_position_dv01,
      },
    },
  },
];
// Scenario P&L: the host-parity book at t0 under its two parity scenarios
// (portfolio.scenarioPnlBatch, scenario texts joined into one JSON array as
// the worker does), and the first scenario through applyScenarioAndRevalue.
function scenarioPnl(specJson, marketJson, scenarioJsons) {
  const book = native.Portfolio.fromSpec(specJson);
  const market = native.MarketContext.fromJson(marketJson);
  try {
    return {
      request: { scenarioJsons },
      batch: {
        positionIds: book.positionIds,
        items: native.scenarioPnlBatch(
          book,
          `[${scenarioJsons.join(",")}]`,
          market,
        ),
      },
      revalue: {
        scenarioJson: scenarioJsons[0],
        positionIds: book.positionIds,
        entityIds: book.entityIds,
        view: native.applyScenarioAndRevalue(book, scenarioJsons[0], market),
      },
    };
  } finally {
    book.free();
    market.free();
  }
}
cases.push({
  file: "scenario-pnl.json",
  value: {
    ...scenarioPnl(
      JSON.stringify(inputs.portfolio_spec),
      JSON.stringify(inputs.market_t0),
      inputs.scenarios.map((scenario) => JSON.stringify(scenario)),
    ),
    // The batch pinned by the Py/WASM host-parity tests.
    expected: { scenario_pnl_batch: expected.scenario_pnl_batch },
  },
});
for (const { file, value } of cases)
  await emit(new URL(file, import.meta.url), serializeHost(value));
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native portfolio fixtures.`,
);
