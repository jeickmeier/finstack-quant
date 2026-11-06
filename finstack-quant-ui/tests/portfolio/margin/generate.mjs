import { checking, emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoSource } from "../../../scripts/fixture-inputs.mjs";
import { serializeHost } from "../../../src/codec.mjs";
// The interest_rate_swap and deposit pricing cases (no patches) over the
// shared pricing market at their valuation date.
const pricing = await readRepoSource(
  "finstack-quant-ui/tests/valuations/instruments/pricing-cases.json",
);
const caseOf = (type) =>
  pricing.value.cases.find((entry) => entry.type === type);
const swapCase = caseOf("interest_rate_swap");
const depositCase = caseOf("deposit");
for (const entry of [swapCase, depositCase])
  if (entry.instrumentPatches.length || entry.marketPatches.length)
    throw new Error(`${entry.type} pricing case gained patches`);
if (swapCase.request.asOf !== depositCase.request.asOf)
  throw new Error("swap and deposit pricing cases moved apart");
const swap = await readRepoSource(swapCase.instrumentSource.path);
const deposit = await readRepoSource(depositCase.instrumentSource.path);
const market = await readRepoSource(
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
);
const asOf = swapCase.request.asOf;
// Two bilateral netting sets from the Rust regulatory CSA presets; the
// deposit carries no margin_spec, so Rust counts it without margin.
const usdCsa = native.csaSpecUsdRegulatory();
const csaB = native.csaSpecRegulatory("USD", "CSA-B", "USD-OIS");
const margined = (id, csa) => ({
  ...swap.value.instrument,
  spec: {
    ...swap.value.instrument.spec,
    id,
    margin_spec: { clearing_status: "bilateral", csa },
  },
});
const spec = {
  id: "MARGIN-BOOK",
  name: "Margined rates book",
  base_currency: "USD",
  as_of: asOf,
  entities: { RATES: { id: "RATES", name: "Rates" } },
  positions: [
    {
      position_id: "IRS-A",
      entity_id: "RATES",
      instrument_id: "IRS-A",
      instrument_spec: margined("IRS-A", usdCsa),
      quantity: 1,
      unit: "units",
    },
    {
      position_id: "IRS-B",
      entity_id: "RATES",
      instrument_id: "IRS-B",
      instrument_spec: margined("IRS-B", csaB),
      quantity: 2,
      unit: "units",
    },
    {
      position_id: "DEP-1",
      entity_id: "RATES",
      instrument_id: deposit.value.instrument.spec.id,
      instrument_spec: deposit.value.instrument,
      quantity: 1,
      unit: "units",
    },
  ],
};
const specJson = JSON.stringify(spec);
const marketJson = JSON.stringify(market.value);
// The worker's PortfolioApi.portfolioMargin over the raw package.
function run(currentImCollateral) {
  const book = native.Portfolio.fromSpec(specJson);
  const context = native.MarketContext.fromJson(marketJson);
  try {
    return {
      ok: true,
      result: native.portfolioMargin(
        book,
        context,
        asOf,
        currentImCollateral ?? undefined,
      ),
    };
  } catch (error) {
    return { ok: false, kind: error.kind, message: error.message };
  } finally {
    book.free();
    context.free();
  }
}
const cases = [
  { name: "no-collateral", currentImCollateral: null },
  {
    name: "usd-collateral-held",
    currentImCollateral: {
      [usdCsa.id]: { amount: "1000000", currency: "USD" },
    },
  },
  {
    name: "unknown-csa",
    currentImCollateral: { "NO-SUCH-CSA": { amount: "1", currency: "USD" } },
  },
].map((entry) => ({ ...entry, ...run(entry.currentImCollateral) }));
await emit(
  new URL("cases.json", import.meta.url),
  serializeHost({
    description:
      "Native portfolio margin (portfolio.portfolioMargin) of a book with two interest rate swaps margined bilaterally under the Rust USD regulatory CSA and a regulatory CSA-B (quantity 2), and an unmargined deposit, over the pricing market; without collateral, with 1,000,000 USD held under the USD regulatory CSA, and with collateral for an unknown CSA (native validation error). The market is not stored.",
    sources: [pricing.source, swap.source, deposit.source, market.source],
    asOf,
    specJson,
    cases,
  }),
);
console.log(
  `${checking ? "Checked" : "Generated"} ${cases.length} native portfolio margin runs.`,
);
