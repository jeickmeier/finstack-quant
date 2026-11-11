import { emit, sha256 } from "../../../scripts/fixture-output.mjs";
import {
  native,
  readRepoJson,
  readRepoText,
} from "../../../scripts/fixture-inputs.mjs";
// The CDX base-correlation calibration is the slowest native run behind the
// fixtures, so this reuses the market tests/calibration/generate.mjs (run
// first) recorded instead of calibrating the same input again.
const source =
  "finstack-quant/calibration/examples/market_bootstrap/05_cdx_base_correlation.json";
const input = await readRepoText(source);
const calibration = (
  await readRepoJson("finstack-quant-ui/tests/calibration/cases.json")
).find((entry) => entry.source === source && entry.variant === undefined);
if (calibration?.sha256 !== sha256(input))
  throw new Error(
    `tests/calibration/cases.json does not hold the current ${source}; run \`mise run ui-fixtures\`.`,
  );
const calibrated = calibration.result.result.final_market;
const variants = await readRepoJson(
  "finstack-quant-ui/registry/core/components/curve-link-example/market.json",
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
  prices: { ...calibrated.prices, "USD/CSA": { unitless: 1.25 } },
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
      pivot_currency: "USD",
      cache_capacity: 17,
    },
    quotes: [["EUR", "USD", 1.1]],
    provider_quotes: [
      ["EUR", "USD", 1.12],
      ["JPY", "USD", 0.007],
    ],
    provider_pinned_quotes: [["EUR", "USD", "2026-05-08", "period_end", 1.14]],
    pinned_quotes: [
      ["EUR", "USD", "2026-05-08", "period_end", 1.13],
      ["EUR", "USD", "2026-05-08", "cashflow_date", 1.11],
    ],
  },
};
const handle = native.MarketContext.fromJson(JSON.stringify(supplemental));
try {
  await emit(
    new URL("./cases.json", import.meta.url),
    JSON.stringify({
      source,
      sha256: sha256(input),
      calibrated,
      supplemental: JSON.parse(handle.toJson()),
    }),
  );
} finally {
  handle.free();
}
