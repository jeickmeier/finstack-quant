import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const paths = [
  "finstack-quant/valuations/tests/instruments/json_examples/term_loan.json",
  "finstack-quant/valuations/tests/instruments/json_examples/revolving_credit.json",
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
];
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, repository), "utf8"));
const [termLoan, revolver, baseMarket] = await Promise.all(paths.map(read));
const asOf = "2023-12-28";
// Seasoned floating coupons read their past resets from this series.
const market = JSON.stringify({
  ...baseMarket,
  series: [
    {
      id: "FIXING:USD-SOFR-3M",
      observations: [
        ["2023-06-29", 0.052],
        ["2023-09-29", 0.0535],
        ["2023-12-28", 0.0535],
      ],
      interpolation: "step",
    },
  ],
});
const withSpec = (instrument, spec) => ({
  ...instrument,
  instrument: {
    ...instrument.instrument,
    spec: { ...instrument.instrument.spec, ...spec },
  },
});
// A seasoned term loan B: SOFR + 375 with a 50 bp floor, 1.5% upfront fee, a
// soft call at 101 stepping to par a year later, and 0.25% quarterly amortization.
const callable = withSpec(termLoan, {
  id: "TLB-USD-2028",
  issue_date: "2023-07-03",
  maturity: "2028-07-03",
  amortization: { percent_of_original_per_period: { pct: 0.0025 } },
  rate: {
    floating: {
      forward_curve_id: "USD-SOFR-3M",
      spread_bp: "375",
      gearing: "1",
      gearing_includes_spread: true,
      reset_frequency: { count: 3, unit: "months" },
      reset_lag_days: 2,
      index_floor_bp: "50",
    },
  },
  call_schedule: {
    calls: [
      { date: "2024-07-03", price_pct_of_par: 101, call_type: "soft" },
      { date: "2025-07-03", price_pct_of_par: 100 },
    ],
  },
  oid_eir: { include_fees: true },
  upfront_fee: { fraction_of_commitment: 0.015 },
});
const quoted = withSpec(callable, {
  instrument_pricing_overrides: {
    market_quotes: { quoted_clean_price_pct: 100.5 },
  },
});
// The card's ids; loan-yield-card.tsx holds the same list, pinned by its test.
const ids = [
  "ytm",
  "ytw",
  "ytc",
  "yt2y",
  "yt3y",
  "yt4y",
  "discount_margin",
  "all_in_rate",
  "oid_eir_rate",
  "oid_eir_amortization",
  "embedded_option_value",
  "recovery01",
  "jump_to_default",
];
function metricValues(request) {
  return request.metrics.map((metric) => {
    try {
      return {
        metric,
        value: {
          ok: true,
          value: native.priceInstrument(
            request.instrumentJson,
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
function card(type, instrument, model) {
  const available = native.metricsForInstrument(type);
  const listed = Object.values(available).flat();
  const metrics = ids.filter((id) => listed.includes(id));
  const request = {
    instrumentJson: JSON.stringify(instrument),
    marketJson: market,
    asOf,
    model,
    metrics,
  };
  return {
    type,
    available,
    request,
    metadata: native.metricMetadata(metrics),
    rows: metricValues(request),
  };
}
const cases = {
  // Every id computes: a quote gives the discount margin, the tree the call value.
  quotedTree: card("term_loan", quoted, "tree"),
  // The call value needs the tree model; the rest of the card still shows.
  quotedDiscounting: card("term_loan", quoted, "discounting"),
  // A callable loan needs a quote for its discount margin; yields use the model price.
  modelTree: card("term_loan", callable, "tree"),
  // A fixed loan has no discount margin.
  fixed: card("term_loan", termLoan, "discounting"),
  // Rust lists seven of the card's ids for a revolver.
  revolver: card("revolving_credit", revolver, "discounting"),
  // A credit curve under the tree model prices on it, so the default-risk ids price.
  hazard: card(
    "term_loan",
    withSpec(termLoan, { credit_curve_id: "ACME-HAZARD" }),
    "tree",
  ),
};
// What one failing id does to a request naming several: the whole call fails.
let wholeRequestError = null;
try {
  native.priceInstrument(
    cases.fixed.request.instrumentJson,
    market,
    asOf,
    "discounting",
    ["ytm", "discount_margin", "all_in_rate"],
  );
} catch (error) {
  wholeRequestError = error.message;
}
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
      "Native metric values of a callable floating term loan, a fixed term loan and a revolver, each id priced on its own call, with the native error of an id Rust rejects and the error of one request naming several ids.",
    sources,
    wholeRequestError,
    cases,
  }),
);
