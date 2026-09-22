import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const source =
  "finstack-quant/calibration/examples/market_bootstrap/12_full_credit_desk_market.json";
const input = await readFile(
  new URL(`../../../../${source}`, import.meta.url),
  "utf8",
);
const calibrated = native.calibrate(input).result.final_market;
const variants = JSON.parse(
  await readFile(
    new URL("../../../src/fixtures/curves/market.json", import.meta.url),
    "utf8",
  ),
);
const curves = [
  ...calibrated.curves,
  ...variants.curves.filter(
    (v) => !calibrated.curves.some((c) => c.type === v.type && c.id === v.id),
  ),
];
const discount = {
  ...variants.curves.find((c) => c.type === "discount"),
  id: "NEG-RATES",
  allow_non_monotonic: true,
  min_forward_rate: -1,
  knot_points: [
    [0, 1],
    [1, 1.01],
    [5, 1.04],
  ],
};
const supplemental = {
  ...calibrated,
  curves: [...curves, discount],
  collateral: { "USD/CSA": "NEG-RATES" },
  hierarchy: {
    roots: {
      Rates: {
        tags: { desk: "Credit" },
        children: {
          Negative: { curves: ["NEG-RATES"], tags: { policy: "nonstandard" } },
        },
      },
    },
  },
  fx: {
    config: {
      enable_triangulation: true,
      pivot_currency: "USD",
      cache_capacity: 17,
    },
    quotes: [["EUR", "USD", 1.1]],
    provider_quotes: [
      ["EUR", "USD", 1.12],
      ["JPY", "USD", 0.007],
    ],
    pinned_quotes: [
      ["EUR", "USD", "2026-05-08", "period_end", 1.13],
      ["EUR", "USD", "2026-05-08", "cashflow_date", 1.11],
    ],
  },
};
const handle = new native.Market(JSON.stringify(supplemental));
try {
  await writeFile(
    new URL("./cases.json", import.meta.url),
    JSON.stringify(
      {
        source,
        sha256: createHash("sha256").update(input).digest("hex"),
        calibrated,
        supplemental: JSON.parse(handle.toJson()),
      },
      null,
      2,
    ) + "\n",
  );
} finally {
  handle.free();
}
