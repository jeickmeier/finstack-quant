import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const paths = [
  "finstack-quant/valuations/tests/instruments/json_examples/bond.json",
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
];
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, repository), "utf8"));
const [bullet, baseMarket] = await Promise.all(paths.map(read));
const asOf = "2024-07-15";
// The shared market's discount curves are identical, so a government curve that
// differs needs its own knots: g_spread and i_spread then tell apart.
const treasury = baseMarket.curves.find((curve) => curve.id === "USD-TREASURY");
const marketJson = JSON.stringify({
  ...baseMarket,
  curves: [
    ...baseMarket.curves,
    {
      ...treasury,
      id: "USD-GOVT",
      knot_points: [
        [0, 1],
        [1, 0.975],
        [40, 0.35],
      ],
    },
  ],
});
const withSpec = (bond, spec) => ({
  ...bond,
  instrument: {
    ...bond.instrument,
    spec: { ...bond.instrument.spec, ...spec },
  },
});
const window = (date, price_pct_of_par) => ({
  start: date,
  end: date,
  price_pct_of_par,
});
const threeMonths = { count: 3, unit: "months" };
const horizon = (target) =>
  JSON.stringify({
    theta_period: threeMonths,
    breakeven_config: { target, mode: "linear" },
  });
// A five-year non-call: the tree model, on rates alone. Rates-credit tree metrics
// take minutes each, so no fixture prices on that model.
const callable = withSpec(bullet, {
  maturity: "2029-01-15",
  call_put: {
    calls: [window("2026-01-15", 102), window("2028-01-15", 101)],
    puts: [],
  },
  instrument_pricing_overrides: {
    model_config: { hw1f_mean_reversion: 0.03, hw1f_sigma: 0.01 },
    market_quotes: { quoted_clean_price_pct: 101 },
  },
});
const exampleFloating = native.Bond.exampleFloating();
// Issued on the valuation date, so no past fixing is needed.
const frn = withSpec(JSON.parse(exampleFloating.toJson()), {
  issue_date: asOf,
  instrument_pricing_overrides: {
    market_quotes: { quoted_clean_price_pct: 99.5 },
  },
});
exampleFloating.free();
// The card's ids; bond-credit-card.tsx holds the same list, pinned by its test.
const ids = [
  "z_spread",
  "i_spread",
  "g_spread",
  "oas",
  "asw_par",
  "asw_market",
  "discount_margin",
  "embedded_option_value",
  "duration_mod",
  "duration_mac",
  "convexity",
  "dv01",
  "cs01",
  "recovery01",
  "jump_to_default",
  "theta",
  "theta_carry",
  "theta_roll_down",
  "carry_total",
  "coupon_income",
  "pull_to_par",
  "roll_down",
  "funding_cost",
  "theta_period_days",
  "breakeven",
];
function metricValues(request) {
  const instrument = native.validateInstrumentJson(
    request.instrumentJson,
    request.metricPricingOverrides,
  );
  return request.metrics.map((metric) => {
    try {
      return {
        metric,
        value: {
          ok: true,
          value: native.priceInstrument(
            instrument,
            request.marketJson,
            request.asOf,
            request.model,
            [metric],
          ).measures[metric],
        },
      };
    } catch (error) {
      return {
        metric,
        value: {
          ok: false,
          error: { name: error.name, message: error.message, kind: error.kind },
        },
      };
    }
  });
}
const available = native.metricsForInstrument("bond");
const listed = Object.values(available).flat();
const metrics = ids.filter((id) => listed.includes(id));
const metadata = native.metricMetadata(metrics);
function card(instrument, model, metricPricingOverrides) {
  const request = {
    instrumentJson: JSON.stringify(instrument),
    marketJson,
    asOf,
    model,
    metrics,
    ...(metricPricingOverrides === undefined ? {} : { metricPricingOverrides }),
  };
  return { request, rows: metricValues(request) };
}
const cases = {
  // No quote, curve or config: spreads to the model price, and each missing input
  // is that id's own native error.
  bullet: card(bullet, "discounting"),
  // A quote and a government curve over three months: g_spread and breakeven price.
  quoted: card(
    withSpec(bullet, {
      instrument_pricing_overrides: {
        model_config: { government_curve_id: "USD-GOVT" },
        market_quotes: { quoted_clean_price_pct: 99 },
      },
    }),
    "discounting",
    horizon("z_spread"),
  ),
  // OAS and the call value need the tree; the breakeven here is in yield.
  callable: card(callable, "tree", horizon("ytm")),
  // Hazard-priced: recovery01 and jump to default price, while CS01 needs a
  // calibration recipe this curve lacks, and so does a z-spread breakeven over it.
  hazard: card(
    withSpec(bullet, { credit_curve_id: "ACME-HAZARD" }),
    "hazard_rate",
    horizon("z_spread"),
  ),
  // A floating coupon has a discount margin and no fixed-rate quote.
  frn: card(frn, "discounting", horizon("ytm")),
};
const sources = await Promise.all(
  paths.map(async (path) => ({
    path,
    sha256: createHash("sha256")
      .update(await readFile(new URL(path, repository)))
      .digest("hex"),
  })),
);
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native metric values of a bullet, a quoted bond with a government curve, a callable under the tree model, a hazard-priced bond and a floating-rate note, each id priced on its own call under the case's MetricPricingOverrides, with the native error of each id Rust rejects.",
    sources,
    type: "bond",
    available,
    metadata,
    cases,
  }),
);
