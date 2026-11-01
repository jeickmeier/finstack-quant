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
const marketJson = JSON.stringify(baseMarket);
const asOf = "2024-07-15";
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
const hullWhite = { hw1f_mean_reversion: 0.03, hw1f_sigma: 0.01 };
const callable = withSpec(bullet, {
  call_put: {
    calls: [window("2026-01-15", 102), window("2028-01-15", 101)],
    puts: [],
  },
  instrument_pricing_overrides: { model_config: hullWhite },
});
const exampleFloating = native.Bond.exampleFloating();
// Issued on the valuation date, so no past fixing is needed.
const frn = withSpec(JSON.parse(exampleFloating.toJson()), {
  issue_date: asOf,
});
exampleFloating.free();
const compute = (spec, quoteInput) => {
  const instrumentJson = JSON.stringify(spec);
  const bond = native.Bond.fromJson(instrumentJson);
  try {
    return {
      request: { instrumentJson, marketJson, asOf, quoteInput },
      quotes: native.computeQuotes(bond, marketJson, asOf, quoteInput),
    };
  } finally {
    bond.free();
  }
};
/** The native set of one quote, with what priceInstrument measures at its clean price. */
function quoteCase(spec, quoteInput, metrics) {
  const entry = compute(spec, quoteInput);
  const quoted = withSpec(spec, {
    instrument_pricing_overrides: {
      ...spec.instrument.spec.instrument_pricing_overrides,
      market_quotes: { quoted_clean_price_pct: entry.quotes.clean_price_pct },
    },
  });
  return {
    ...entry,
    metrics: native.priceInstrument(
      JSON.stringify(quoted),
      marketJson,
      asOf,
      "default",
      metrics,
    ).measures,
  };
}
const fixedMetrics = [
  "ytm",
  "ytw",
  "z_spread",
  "oas",
  "asw_par",
  "asw_market",
  "i_spread",
  "japanese_simple_yield",
  "moosmuller_ytm",
];
const clean = quoteCase(bullet, { clean_price_pct: 99 }, fixedMetrics);
const cases = {
  clean,
  // Back from the yield of the first case: the same price within the solver's tolerance.
  fromYield: quoteCase(bullet, { ytm: clean.quotes.ytm }, fixedMetrics),
  // At a premium the call is worst, so ytw sits below ytm.
  callable: quoteCase(callable, { clean_price_pct: 106 }, fixedMetrics),
  // A floating coupon has a discount margin and no fixed-rate quote.
  frn: quoteCase(frn, { discount_margin: 0.01 }, [
    "ytm",
    "ytw",
    "z_spread",
    "discount_margin",
    "oas",
    "i_spread",
    "moosmuller_ytm",
  ]),
};
/** The native error of a request, kept as returned. */
function nativeError(spec, quoteInput) {
  const instrumentJson = JSON.stringify(spec);
  const bond = native.Bond.fromJson(instrumentJson);
  try {
    native.computeQuotes(bond, marketJson, asOf, quoteInput);
    throw new Error("Expected a native error");
  } catch (error) {
    return {
      request: { instrumentJson, marketJson, asOf, quoteInput },
      error: { name: error.name, kind: error.kind, message: error.message },
    };
  } finally {
    bond.free();
  }
}
const errors = {
  // A quote kind the bond does not have.
  discountMarginOnFixed: nativeError(bullet, { discount_margin: 0.01 }),
  // Z-spread is not a quote on a bond with exercise rights: use its OAS.
  zSpreadOnCallable: nativeError(callable, { z_spread: 0.01 }),
  // A call on the quote date below the price has no spread that matches same-day cash.
  quoteDateCall: nativeError(
    withSpec(callable, {
      call_put: { calls: [window("2024-07-16", 99)], puts: [] },
    }),
    { clean_price_pct: 100 },
  ),
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
      "Native computeQuotes results of fixed, callable and floating bonds, each with what priceInstrument measures at its clean price, and the native errors of quotes that cannot be solved.",
    sources,
    metadata: native.metricMetadata([
      "clean_price",
      "dirty_price",
      "ytm",
      "ytw",
      "z_spread",
      "discount_margin",
      "oas",
      "asw_par",
      "asw_market",
      "i_spread",
      "japanese_simple_yield",
      "moosmuller_ytm",
    ]),
    errors,
    cases,
  }),
);
