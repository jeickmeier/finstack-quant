import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { serializeHost } from "../../../src/codec.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const source = async (path) => {
  const bytes = await readFile(new URL(path, repository));
  return {
    value: JSON.parse(bytes),
    source: { path, sha256: createHash("sha256").update(bytes).digest("hex") },
  };
};
// The margined rates book the margin fixture writes (two bilateral swaps and
// an unmargined deposit), over the pricing market on that fixture's date,
// under the two host-parity USD-OIS scenarios.
const margin = await source(
  "finstack-quant-ui/tests/portfolio/margin/cases.json",
);
const market = await source(
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
);
const parity = await source(
  "finstack-quant/portfolio/tests/fixtures/host_parity.json",
);
const { specJson, asOf } = margin.value;
const marketJson = JSON.stringify(market.value);
// The host's choices for the workbench: the metric menu, the bucketed base
// metric charted on the Risk tab, and the scenario texts in batch order.
const request = {
  asOf,
  metrics: ["dv01", "bucketed_dv01", "theta"],
  seriesBase: "bucketed_dv01",
  scenarioJsons: parity.value.inputs.scenarios.map((scenario) =>
    JSON.stringify(scenario),
  ),
};
// The worker's PortfolioApi over the raw package, one native call each:
// valuePortfolio then aggregateMetrics, portfolioMetricsSeries, metricMetadata
// of the returned measure ids, scenarioPnlBatch and portfolioMargin.
const book = native.Portfolio.fromSpec(specJson);
const context = native.MarketContext.fromJson(marketJson);
let value;
try {
  const valuation = native.valuePortfolio(
    book,
    context,
    undefined,
    request.metrics,
  );
  const metrics = native.aggregateMetrics(valuation, context);
  const keys = Object.values(valuation.position_values).flatMap((position) =>
    Object.keys(position.valuation_result?.measures ?? {}),
  );
  value = {
    description:
      "Native portfolio workbench runs of the margined rates book (two interest rate swaps margined bilaterally and an unmargined deposit) over the pricing market: portfolio.valuePortfolio and aggregateMetrics with the host's metric menu, the bucketed_dv01 series, metricMetadata of the returned measure ids, portfolio.scenarioPnlBatch under the host-parity up_10bp and down_15bp USD-OIS scenarios, and portfolio.portfolioMargin on the margin fixture's date with no collateral held. The market is not stored.",
    sources: [margin.source, market.source, parity.source],
    specJson,
    request,
    report: {
      positionIds: book.positionIds,
      entityIds: book.entityIds,
      valuation,
      metrics,
    },
    series: {
      base: request.seriesBase,
      entries: native.portfolioMetricsSeries(metrics, request.seriesBase),
    },
    metadata: native.metricMetadata([...new Set(keys)]),
    batch: {
      positionIds: book.positionIds,
      items: native.scenarioPnlBatch(
        book,
        `[${request.scenarioJsons.join(",")}]`,
        context,
      ),
    },
    margin: native.portfolioMargin(book, context, asOf, undefined),
  };
} finally {
  book.free();
  context.free();
}
await emit(new URL("cases.json", import.meta.url), serializeHost(value));
console.log(
  `${checking ? "Checked" : "Generated"} the native portfolio workbench fixture.`,
);
