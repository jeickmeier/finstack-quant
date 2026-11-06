import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
import { serializeHost } from "../../../src/codec.mjs";
// The workbench's book (two bilateral swaps and an unmargined deposit) over
// the pricing market on the workbench date, as the Risk tab decomposes it.
const workbench = await readRepoSource(
  "finstack-quant-ui/tests/portfolio/workbench/cases.json",
);
const market = await readRepoSource(
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
);
const parity = await readRepoSource(
  "finstack-quant/portfolio/tests/fixtures/host_parity.json",
);
const { specJson } = workbench.value;
const { asOf } = workbench.value.request;
const marketJson = JSON.stringify(market.value);
// The host's choices: two additive metrics reported per primitive, and the
// host-parity factor model's one factor (a parallel USD-OIS shift in bp).
const inputs = {
  metrics: ["dv01", "theta"],
  factorsJson: JSON.stringify(parity.value.inputs.factor_model_config.factors),
};
// The worker's PortfolioApi.primitiveExposures and computeFactorSensitivities
// over the cached handles, one native call each, over the raw package
// (portfolio.primitiveExposures is the raw portfolioPrimitiveExposures).
const book = native.Portfolio.fromSpec(specJson);
const context = native.MarketContext.fromJson(marketJson);
function failure(call, request) {
  try {
    call();
  } catch (error) {
    return {
      request,
      error: { name: error.name, kind: error.kind, message: error.message },
    };
  }
  throw new Error("Expected a native error");
}
let value;
try {
  const exposures = (metrics) =>
    native.portfolioPrimitiveExposures(book, context, metrics);
  value = {
    description:
      "Native primitive exposures of the portfolio workbench's margined rates book (two interest rate swaps and a deposit) over the pricing market: portfolio.primitiveExposures with the host's additive dv01 and theta and with no metric, portfolio.computeFactorSensitivities against the host-parity usd_rates factor (a parallel USD-OIS shift in bp) on the workbench date with Rust's default bump, and the native errors of a non-additive metric, a metric one position cannot compute and a factor mapped to a curve the market lacks. The market is not stored.",
    sources: [workbench.source, market.source, parity.source],
    asOf,
    inputs,
    report: exposures(inputs.metrics),
    unmeasured: exposures([]),
    sensitivities: native.computeFactorSensitivities(
      book,
      inputs.factorsJson,
      context,
      asOf,
      undefined,
    ),
    errors: {
      // A yield is not additive across primitives.
      nonAdditive: failure(() => exposures(["ytm"]), { metrics: ["ytm"] }),
      // A deposit has no pv01; Rust names the position.
      notApplicable: failure(() => exposures(["pv01"]), {
        metrics: ["pv01"],
      }),
    },
  };
  const missingCurve = JSON.stringify([
    {
      id: "eur_rates",
      factor_type: "rates",
      market_mapping: {
        curve_parallel: { curve_ids: ["EUR-ESTR"], units: "rate_bp" },
      },
    },
  ]);
  value.errors.missingCurve = failure(
    () =>
      native.computeFactorSensitivities(
        book,
        missingCurve,
        context,
        asOf,
        undefined,
      ),
    { factorsJson: missingCurve },
  );
} finally {
  book.free();
  context.free();
}
await emit(new URL("cases.json", import.meta.url), serializeHost(value));
console.log(
  `${checking ? "Checked" : "Generated"} the native primitive exposure fixture.`,
);
