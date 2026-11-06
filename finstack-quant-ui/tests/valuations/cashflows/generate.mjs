import { emit } from "../../../scripts/fixture-output.mjs";
import {
  native,
  readRepoJson,
  sourceRecord,
} from "../../../scripts/fixture-inputs.mjs";
import { priceExtras } from "../../shared/price-extras.mjs";
const manifestPath =
  "finstack-quant-ui/tests/valuations/instruments/pricing-cases.json";
const bondPath = "finstack-quant-ui/src/fixtures/results/bond.json";
const manifest = await readRepoJson(manifestPath);
// Pinned inputs: the JSON files this fixture is built from. The manifest also
// supplies request fields and patches; drift there is caught by `--check`, not
// by this pin list, so its other cases do not invalidate these flows.
const paths = new Set([bondPath]);
async function source(source) {
  paths.add(source.path);
  let value = await readRepoJson(source.path);
  for (const key of source.pointer) value = value[key];
  return value;
}
function patch(value, patches) {
  for (const { path, value: next } of patches) {
    let target = value;
    for (const key of path.slice(0, -1)) target = target[key];
    target[path.at(-1)] = next;
  }
  return value;
}
const inputs = [
  { type: "bond", request: (await readRepoJson(bondPath)).request },
];
// A PIK loan shows how the balance accretes; the agency pool shows per-period balances.
const pikLoan = [
  { path: ["instrument", "spec", "id"], value: "TERM-LOAN-USD-5Y-PIK" },
  { path: ["instrument", "spec", "coupon_type"], value: "pik" },
  { path: ["instrument", "spec", "amortization"], value: "none" },
];
// A one-year remaining pool: enough periods to show the balance, not 1,014 rows.
const shortPool = [
  { path: ["instrument", "spec", "maturity"], value: "2024-12-28" },
  { path: ["instrument", "spec", "wam_months"], value: 12 },
];
for (const [type, manifestType, extraPatches] of [
  ["fx_swap", "fx_swap", []],
  ["xccy_swap", "xccy_swap", []],
  ["equity_option", "equity_option", []],
  ["term_loan_pik", "term_loan", pikLoan],
  ["agency_mbs_passthrough", "agency_mbs_passthrough", shortPool],
]) {
  const entry = manifest.cases.find((c) => c.type === manifestType);
  inputs.push({
    type,
    request: {
      ...entry.request,
      instrumentJson: JSON.stringify(
        patch(await source(entry.instrumentSource), [
          ...entry.instrumentPatches,
          ...extraPatches,
        ]),
      ),
      marketJson: JSON.stringify(
        patch(await source(entry.marketSource), entry.marketPatches),
      ),
    },
  });
}
const cases = inputs.map(({ type, request }) => {
  const priced = native.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    request.asOf,
    request.model,
    request.metrics,
    priceExtras(request),
  );
  let cashflows = null,
    error = null;
  try {
    cashflows = native.instrumentCashflows(
      request.instrumentJson,
      request.marketJson,
      request.asOf,
      request.model,
    );
  } catch (failure) {
    error = failure.message;
  }
  return {
    type,
    request,
    pricedValue: priced.value,
    cashflows,
    ladder: cashflows && native.calendarYearLadderByCurrency(cashflows),
    error,
  };
});
const sources = await Promise.all([...paths].map(sourceRecord));
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native cashflow envelope objects and their native calendar-year ladders per row currency; no inferred row schema or financial reconciliation.",
    sources,
    cases,
  }),
);
