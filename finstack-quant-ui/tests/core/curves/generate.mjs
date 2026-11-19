import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
import { pricingCases } from "../../browser/cases.mjs";
const market = JSON.parse((await pricingCases()).bond.marketJson);
const base = { base: "2025-01-01", day_count: "act_365f" };
// Factor curves (discount, forward projection factors, CPI) use a log-space
// style whose extended tail holds the last forward rate; price curves hold the
// boundary value.
const factor = { interp_style: "log_linear", extrapolation: "extend" };
const level = { interp_style: "linear", extrapolation: "flat" };
// Explicit representation fixtures. Native Market validates and serializes them; no samples are evaluated.
market.curves.push(
  {
    ...base,
    ...factor,
    type: "discount",
    id: "OVERLAY",
    knot_points: [
      [0, 1],
      [1, 0.9512345678901234],
      [5, 0.8],
    ],
  },
  {
    ...base,
    ...factor,
    type: "forward",
    id: "FORWARD",
    index_tenor: { count: 3, unit: "months" },
    knot_points: [
      [0, 1],
      [1, 0.9579],
      [5, 0.8228],
    ],
  },
  {
    ...factor,
    type: "inflation",
    id: "CPI",
    base_date: base.base,
    day_count: base.day_count,
    knot_points: [
      [0, 300],
      [1, 307.5],
      [5, 340],
    ],
  },
  {
    type: "base_correlation",
    id: "CORRELATION",
    detachment_points: [0.03, 0.07, 0.1],
    correlations: [0.2, 0.3, 0.4],
  },
  ...[
    {
      type: "price",
      id: "PRICE",
      level_kind: "price",
      knot_points: [
        [0, 100],
        [1, 102],
        [5, 110],
      ],
    },
    {
      type: "price",
      id: "VOL_INDEX",
      level_kind: "vol_index",
      knot_points: [
        [0, 18],
        [1, 19],
        [5, 20],
      ],
    },
  ].map((curve) => ({
    ...base,
    ...level,
    ...curve,
  })),
  {
    type: "parametric",
    id: "NS",
    base_date: base.base,
    day_count: base.day_count,
    model: {
      variant: "ns",
      beta0: 0.04,
      beta1: 0.003,
      beta2: -0.006,
      tau: 1.5,
    },
  },
);
const state = native.MarketContext.fromJson(JSON.stringify(market));
try {
  await emit(
    new URL(
      "../../../registry/core/components/curve-link-example/market.json",
      import.meta.url,
    ),
    state.toJson(),
  );
} finally {
  state.free();
}
console.log(
  "Retained native-validated state for all seven curve storage types.",
);
