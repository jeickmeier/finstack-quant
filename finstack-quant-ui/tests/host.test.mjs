import { expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import init, {
  core,
  models,
  valuations,
} from "../../finstack-quant-wasm/index.js";
import { adaptValuation, exportValuation } from "../src/host";
import { valuationCodec } from "./details/restore";

await init({
  module_or_path: await readFile(
    new URL(
      "../../finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm",
      import.meta.url,
    ),
  ),
});
const root = new URL("../../finstack-quant/valuations/tests/", import.meta.url);
const read = async (path) =>
  JSON.parse(await readFile(new URL(path, root), "utf8"));
const regressions = await read("fixtures/valuation_binding_regressions.json");
const credit = await read("fixtures/production_credit_tranche.json");
const creditMarket = (await read("fixtures/production_cds_option.json")).market;
const sc = await read("fixtures/production_structured_credit.json");
sc.instrument.instrument.spec.instrument_pricing_overrides = {
  model_config: { mc_paths: 1 },
};
const bond = await read("instruments/json_examples/bond.json");
Object.assign(bond.instrument.spec, {
  discount_curve_id: "USD-OIS",
  credit_curve_id: "HZ",
  issue_date: "2025-01-01",
  maturity: "2026-01-01",
  instrument_pricing_overrides: {
    model_config: { hazard_volatility: 0.01, mc_paths: 8, tree_steps: 4 },
  },
});
const market = structuredClone(regressions.market);
market.curves = ["USD-OIS", "EUR-OIS"].map((id) => ({
  ...regressions.market.curves.find((curve) => curve.type === "discount"),
  id,
  base: "2024-01-01",
}));
market.fx = {
  config: { pivot_currency: "USD", enable_triangulation: true },
  quotes: [["EUR", "USD", 1.1]],
  pinned_quotes: [],
  provider_quotes: [],
};
const fx = await read("instruments/json_examples/fx_forward.json");
const fxSwap = await read("instruments/json_examples/fx_swap.json");
const price = (instrument, market, asOf, model) =>
  valuations.instruments.priceInstrument(
    JSON.stringify(instrument),
    JSON.stringify(market),
    asOf,
    model,
    [],
  );
const equity = (id, price) => ({
  type: "equity",
  spec: {
    id,
    ticker: id,
    currency: "USD",
    shares: 1,
    price_quote: price,
    price_id: null,
    div_yield_id: null,
    discrete_dividends: [],
    discount_curve_id: "USD",
    attributes: {},
  },
});
const composite = valuations.composite.initialize(
  {
    id: "UI-COMPOSITE",
    reporting_currency: "USD",
    capital: { amount: "100", currency: "USD" },
    legs: [
      { instrument_id: "A", instrument: equity("A", 100), weight: 1 },
      { instrument_id: "B", instrument: equity("B", 90), weight: -1 },
    ],
    weighting_method: { kind: "fixed_quantity" },
    rebalance_rule: { kind: "manual" },
    attributes: {},
  },
  market,
  "2024-01-01",
);
const results = [
  price(bond, creditMarket, "2025-01-01", "rates_credit"),
  price(credit.instrument, credit.market, credit.as_of, "default"),
  price(sc.instrument, sc.market, sc.as_of, "structured_credit_stochastic"),
  price(fx, market, "2024-01-01", "discounting"),
  price(composite.instrument, market, "2024-01-01", "default"),
];

it("covers every canonical detail variant through actual facade pricing", async () => {
  const schema = await readFile(
    new URL("../src/generated/schemas/valuation_result.json", import.meta.url),
    "utf8",
  );
  const variants = Object.values(JSON.parse(schema).$defs)
    .find((node) =>
      node.oneOf?.some(
        (variant) => variant.properties?.type?.const === "monte_carlo",
      ),
    )
    .oneOf.map((variant) => variant.properties.type.const)
    .sort();
  expect(results.map((result) => result.details.type).sort()).toEqual(variants);
  for (const result of results) {
    const clone = structuredClone(result);
    expect(clone).toEqual(result);
    expect(() => adaptValuation(result), result.details.type).not.toThrow();
    expect(adaptValuation(result)).toEqual(result);
    const text = exportValuation(clone, valuations.validateValuationResultJson);
    expect(
      valuationCodec.stringify(
        valuationCodec.parse(text),
        valuations.validateValuationResultJson,
      ),
    ).toBe(text);
  }
});
it("preserves the actual derived seed above 2^53 through clone and Rust export", () => {
  const result = results[0];
  expect(typeof result.details.data.seed).toBe("bigint");
  expect(result.details.data.seed).toBeGreaterThan(
    BigInt(Number.MAX_SAFE_INTEGER),
  );
  expect(typeof result.details.data.estimator_paths).toBe("number");
  expect(Array.isArray(result.details.data.time_grid)).toBe(true);
  const canonical = exportValuation(
    structuredClone(result),
    valuations.validateValuationResultJson,
  );
  expect(valuationCodec.parse(canonical).details.data.seed).toBe(
    result.details.data.seed,
  );
  for (const seed of [
    Number(result.details.data.seed),
    result.details.data.seed.toString(),
  ]) {
    const broken = structuredClone(result);
    broken.details.data.seed = seed;
    expect(() =>
      exportValuation(broken, valuations.validateValuationResultJson),
    ).toThrow();
  }
});
it("tests u64::MAX as a transport boundary on an actual result, not a repricing claim", () => {
  const boundary = structuredClone(results[0]);
  boundary.details.data.seed = (1n << 64n) - 1n;
  const text = exportValuation(
    structuredClone(boundary),
    valuations.validateValuationResultJson,
  );
  expect(text).toContain('"seed":18446744073709551615');
  expect(valuationCodec.parse(text).details.data.seed).toBe((1n << 64n) - 1n);
  expect(() =>
    valuations.validateValuationResultJson(
      text.replace(
        '"seed":18446744073709551615',
        '"seed":"18446744073709551615"',
      ),
    ),
  ).toThrow();
});
it("transports original bond and mixed-currency cashflow text byte for byte", () => {
  for (const instrument of [bond, fxSwap]) {
    const text = valuations.instruments.instrumentCashflowsJson(
      JSON.stringify(instrument),
      JSON.stringify(instrument === bond ? creditMarket : market),
      instrument === bond ? "2025-01-01" : "2024-01-01",
      "discounting",
    );
    expect(typeof text).toBe("string");
    expect(Buffer.from(structuredClone({ text }).text)).toEqual(
      Buffer.from(text),
    );
    if (instrument === fxSwap) {
      // Test evidence only: production views retain the opaque text.
      expect(
        new Set(JSON.parse(text).flows.map((flow) => flow.currency)),
      ).toEqual(new Set(["EUR", "USD"]));
    }
  }
  expect(() =>
    valuations.instruments.instrumentCashflowsJson(
      JSON.stringify(fxSwap),
      JSON.stringify(market),
      "2024-01-01",
      "invalid",
    ),
  ).toThrow();
});
it("preserves the scoped published Float64Array return through structured clone", () => {
  const surface = new core.FxDeltaVolSurface(
    "EURUSD-VOL",
    [1],
    [0.12],
    [0.01],
    [0.002],
  );
  const cube = new core.VolCube(
    "USD-SWAPTION",
    [1],
    [5],
    [0.03, 0.5, -0.2, 0.4, NaN],
    [0.03],
  );
  try {
    const values = models.volatility.getFxDeltaPillarVols(surface, 0);
    const copy = structuredClone(values);
    expect(copy).toBeInstanceOf(Float64Array);
    expect(Buffer.from(copy.buffer)).toEqual(Buffer.from(values.buffer));
    expect(
      Number.isFinite(models.volatility.getCubeVol(cube, 1, 5, 0.03)),
    ).toBe(true);
    expect(
      Number.isFinite(models.volatility.getCubeNormalVol(cube, 1, 5, 0.03)),
    ).toBe(true);
    expect(
      Number.isFinite(models.volatility.getFxDeltaVol(surface, 1, 1.1, 1.1)),
    ).toBe(true);
  } finally {
    surface.free();
    cube.free();
  }
});
