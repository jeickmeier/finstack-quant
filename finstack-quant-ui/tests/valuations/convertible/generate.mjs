import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";
const repository = new URL("../../../../", import.meta.url);
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const paths = [
  "finstack-quant/valuations/tests/fixtures/production_convertible.json",
  "finstack-quant-ui/tests/valuations/instruments/pricing-market.json",
];
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, repository), "utf8"));
const [production, baseMarket] = await Promise.all(paths.map(read));
const asOf = "2024-07-15";
// The share and its volatility on the shared market. A flat hazard curve from
// rates: it has no calibration recipe, so cs01 keeps Rust's error.
const marketJson = JSON.stringify({
  ...baseMarket,
  prices: {
    ...baseMarket.prices,
    "CONV-STOCK": { unitless: 100 },
    "CONV-STOCK-VOL": { unitless: 0.3 },
    "CONV-STOCK-DIVYIELD": { unitless: 0.01 },
  },
});
// A seasoned five-year 2% convertible with a conversion price of 100, between coupons.
const plain = {
  ...production.instrument,
  instrument: {
    ...production.instrument.instrument,
    spec: {
      ...production.instrument.instrument.spec,
      id: "CONV-2029",
      issue_date: "2023-08-15",
      maturity: "2028-08-15",
      notional: { amount: "1000000", currency: "USD" },
      conversion: {
        ...production.instrument.instrument.spec.conversion,
        ratio: 10000,
      },
      cashflow_spec: {
        fixed: {
          ...production.instrument.instrument.spec.cashflow_spec.fixed,
          rate: "0.02",
          stub: "short_front",
        },
      },
      call_put: null,
      credit_curve_id: "ACME-HAZARD",
      recovery_rate: 0.4,
      spot_id: "CONV-STOCK",
      vol_surface_id: "CONV-STOCK-VOL",
      div_yield_id: "CONV-STOCK-DIVYIELD",
    },
  },
};
const withSpec = (spec) => ({
  ...plain,
  instrument: {
    ...plain.instrument,
    spec: { ...plain.instrument.spec, ...spec },
  },
});
// Quoted, so oas and implied_vol solve, and callable at par from year three once
// the share trades at 130% of the conversion price.
const quoted = withSpec({
  call_put: {
    calls: [{ start: "2026-08-15", end: "2028-08-15", price_pct_of_par: 100 }],
    puts: [],
  },
  soft_call_trigger: {
    threshold_pct: 130,
    observation_days: 30,
    required_days_above: 20,
  },
  instrument_pricing_overrides: {
    market_quotes: { quoted_clean_price_pct: 120 },
  },
});
// Delivers a moving share count: Rust gives no hedge ratio or regime.
const mandatory = withSpec({
  conversion: {
    ...plain.instrument.spec.conversion,
    policy: {
      mandatory_variable: {
        conversion_date: "2028-08-15",
        lower_conversion_price: 80,
        upper_conversion_price: 120,
      },
    },
  },
});
// The card's ids; convertible-card.tsx holds the same list, pinned by its test.
const ids = [
  "parity",
  "conversion_value",
  "conversion_premium",
  "bond_floor",
  "delta",
  "gamma",
  "vega",
  "rho",
  "theta",
  "implied_vol",
  "dv01",
  "cs01",
  "oas",
  "dividend01",
  "conversion01",
  "accrued",
  "clean_price",
];
const available = native.metricsForInstrument("convertible_bond");
const listed = Object.values(available).flat();
const metrics = ids.filter((id) => listed.includes(id));
const metadata = native.metricMetadata(metrics);
const model = "tree";
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
function card(instrument, spots) {
  const instrumentJson = JSON.stringify(instrument);
  const request = { instrumentJson, marketJson, asOf };
  const bond = native.ConvertibleBond.fromJson(instrumentJson);
  try {
    return {
      request: { ...request, ...(spots ? { spots } : {}) },
      ladder: native.convertibleSpotLadder(bond, marketJson, asOf, spots),
      metrics: { ...request, model, metrics },
      rows: metricValues({ ...request, model, metrics }),
    };
  } finally {
    bond.free();
  }
}
const cases = {
  plain: card(plain),
  quoted: card(quoted),
  // Share prices the user supplies; Rust sorts them and drops the duplicate.
  supplied: card(plain, [150, 50, 100, 150]),
  mandatory: card(mandatory),
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
      "Native convertible spot ladders and metric values of a plain convertible, a quoted one callable on a soft trigger, one with supplied share prices and a mandatory-variable one, each id priced on its own call under the tree model, with the native error of each id Rust rejects.",
    sources,
    type: "convertible_bond",
    available,
    metadata,
    cases,
  }),
);
