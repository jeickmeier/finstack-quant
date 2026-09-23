import { createRequire } from "node:module";
import { writeFile } from "node:fs/promises";
import { pricingCases } from "../../browser/cases.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const market = JSON.parse((await pricingCases()).bond.marketJson);
const base = {
  base: "2025-01-01",
  day_count: "act_365f",
  interp_style: "linear",
  extrapolation: "flat_forward",
};
// Explicit representation fixtures. Native Market validates and serializes them; no samples are evaluated.
market.curves.push(
  {
    ...base,
    type: "discount",
    id: "OVERLAY",
    allow_non_monotonic: false,
    min_forward_tenor: 0.0001,
    knot_points: [
      [0, 1],
      [1, 0.9512345678901234],
      [5, 0.8],
    ],
  },
  {
    ...base,
    type: "forward",
    id: "FORWARD",
    reset_lag: 2,
    tenor: 0.25,
    knot_points: [
      [0, 0.044],
      [1, 0.041],
      [5, 0.039],
    ],
  },
  {
    type: "inflation",
    id: "CPI",
    base_date: base.base,
    base_cpi: 300,
    interp_style: "linear",
    extrapolation: "flat_forward",
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
  ...["price", "vol_index", "basis_spread"].map((type) => ({
    ...base,
    type,
    id: type.toUpperCase(),
    ...(type === "price"
      ? { spot_price: 100 }
      : type === "vol_index"
        ? { spot_level: 18 }
        : {}),
    knot_points:
      type === "basis_spread"
        ? [
            [0, -0.001],
            [1, -0.0012],
            [5, -0.0015],
          ]
        : type === "vol_index"
          ? [
              [0, 18],
              [1, 19],
              [5, 20],
            ]
          : [
              [0, 100],
              [1, 102],
              [5, 110],
            ],
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
const state = new native.Market(JSON.stringify(market));
try {
  await writeFile(
    new URL(
      "../../../registry/core/components/curve-link-example/market.json",
      import.meta.url,
    ),
    JSON.stringify(JSON.parse(state.toJson()), null, 2) + "\n",
  );
} finally {
  state.free();
}
console.log("Retained native-validated state for all nine curve variants.");
