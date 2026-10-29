import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { checking, emit } from "../../scripts/fixture-output.mjs";
import { serializeHost } from "../../src/codec.mjs";
const repository = new URL("../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// Convertible credit widening 150bp -> 300bp over one day (attribution crate fixture).
const sourcePath =
  "finstack-quant/attribution/tests/fixtures/production_convertible_credit.json";
const sourceBytes = await readFile(new URL(sourcePath, repository));
const source = JSON.parse(sourceBytes);
// The PnlAttribution buckets pnl-attribution-report lists.
const fields = [
  "carry",
  "rates_curves_pnl",
  "credit_curves_pnl",
  "inflation_curves_pnl",
  "correlations_pnl",
  "fx_pnl",
  "fx_translation_pnl",
  "vol_pnl",
  "cross_factor_pnl",
  "model_params_pnl",
  "market_scalars_pnl",
  "residual",
];
const methods = [
  { name: "parallel", method: "parallel" },
  // Rust's default factor order; the waterfall steps chain in it.
  { name: "waterfall", method: { waterfall: native.defaultWaterfallOrder() } },
  { name: "metrics_based", method: "metrics_based" },
  { name: "taylor", method: { taylor: {} } },
];
const cases = methods.map(({ name, method }) => {
  const [attribution] = native.attributePnlMany(
    {
      as_of_t0: source.as_of_t0,
      as_of_t1: source.as_of_t1,
      market_t0: source.market_t0,
      market_t1: source.market_t1,
      method,
    },
    [source.instrument],
  );
  return {
    name,
    method,
    attribution,
    // The Rust helpers a host passes to the report's optional props.
    explain: native.pnlAttributionExplainText(attribution),
    residualWithinTolerance:
      native.pnlAttributionResidualWithinTolerance(attribution),
    pctOfTotal: Object.fromEntries(
      fields.map((field) => [
        field,
        native.pnlAttributionPctOfTotal(
          attribution,
          Number(attribution[field].amount),
        ) ?? null,
      ]),
    ),
  };
});
await emit(
  new URL("cases.json", import.meta.url),
  serializeHost({
    description:
      "Native single-instrument P&L attribution of a convertible bond whose credit spreads widen from 150bp to 300bp over one day, under the four Rust methods (attributePnlMany), with Rust's explain text, residual check and share of total per factor.",
    sources: [
      {
        path: sourcePath,
        sha256: createHash("sha256").update(sourceBytes).digest("hex"),
      },
    ],
    cases,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native P&L attributions.`,
);
