import { beforeAll, afterAll, it, expect } from "vitest";
import { createRequire } from "node:module";
import { startWorker } from "./worker/harness.mjs";
import { QueryClient } from "@tanstack/react-query";
import { pricingCases } from "./browser/cases.mjs";
import { exportValuation } from "../src/host";
import { priceOptions } from "../registry/hooks/use-price-instrument/use-price-instrument";
import { errorValue, unwrap } from "../registry/workers/finstack-contract";
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let proxy, harness;
const requests = await pricingCases();
const history = JSON.stringify({
  base_date: requests.bond.asOf,
  window_days: 2,
  scenarios: [
    {
      date: "2024-12-30",
      shifts: [
        {
          factor: {
            type: "discount_rate",
            curve_id: "USD-OIS",
            tenor_years: 1,
          },
          shift: 0.001,
        },
      ],
    },
    {
      date: "2024-12-31",
      shifts: [
        {
          factor: {
            type: "discount_rate",
            curve_id: "USD-OIS",
            tenor_years: 1,
          },
          shift: -0.0005,
        },
      ],
    },
  ],
});
function direct(request) {
  try {
    return {
      ok: true,
      value: native.priceInstrument(
        request.instrumentJson,
        request.marketJson,
        request.asOf,
        request.model,
        request.metrics,
        request.pricingOptions,
        request.marketHistory,
      ),
    };
  } catch (error) {
    return { ok: false, error: errorValue(error) };
  }
}
function same(actual, expected) {
  if (actual.ok && expected.ok) {
    expect(typeof actual.value.meta.timestamp).toBe("string");
    expected.value.meta.timestamp = actual.value.meta.timestamp;
  }
  expect(actual).toEqual(expected);
}
beforeAll(async () => {
  harness = await startWorker();
  proxy = harness.proxy;
}, 30000);
afterAll(async () => harness?.close());
it("compares real worker results and errors with the direct native facade", async () => {
  for (const request of Object.values(requests))
    same(await proxy.price(request), direct(request));
  const stochastic = unwrap(await proxy.price(requests.stochastic));
  expect(typeof stochastic.details.data.seed).toBe("bigint");
  expect(stochastic.details.data.seed).toBeGreaterThan(
    BigInt(Number.MAX_SAFE_INTEGER),
  );
  const exported = unwrap(await proxy.exportResult(stochastic));
  expect(exported).toContain(`"seed":${stochastic.details.data.seed}`);
  const cashflows = unwrap(await proxy.cashflows(requests.bond));
  expect(cashflows).toBe(
    native.instrumentCashflowsJson(
      requests.bond.instrumentJson,
      requests.bond.marketJson,
      requests.bond.asOf,
      requests.bond.model,
    ),
  );
});
it("keys every pricing input, preserves snapshots, and forwards native history", async () => {
  const query = new QueryClient();
  const client = {
    call: async (method, request) => unwrap(await proxy[method](request)),
  };
  const instrument = JSON.parse(requests.bond.instrumentJson);
  instrument.instrument.spec.id = "WORKER-OTHER";
  const inputs = [
    requests.bond,
    { ...requests.bond, instrumentJson: JSON.stringify(instrument) },
    { ...requests.bond, marketJson: requests.bond.marketJson + " " },
    { ...requests.bond, asOf: "2025-01-02" },
    { ...requests.bond, model: "hazard_rate" },
    { ...requests.bond, metrics: ["dv01"] },
    { ...requests.bond, pricingOptions: '{"theta_period":"1W"}' },
    { ...requests.bond, marketHistory: history },
    { ...requests.bond, metrics: ["hvar"], marketHistory: history },
    { ...requests.bond, model: undefined },
    { ...requests.bond, metrics: undefined },
  ];
  const before = (await proxy.resources()).calls;
  for (const input of inputs) {
    const actual = await query.fetchQuery(priceOptions(client, 1, input));
    same({ ok: true, value: actual }, direct(input));
  }
  expect((await proxy.resources()).calls - before).toBe(inputs.length);
  await query.fetchQuery(
    priceOptions(client, 1, structuredClone(requests.bond)),
  );
  expect((await proxy.resources()).calls - before).toBe(inputs.length);
  const risk = await query.fetchQuery(priceOptions(client, 1, inputs[8]));
  expect(Number.isFinite(risk.measures.hvar)).toBe(true);
  expect(direct({ ...inputs[8], marketHistory: undefined }).ok).toBe(false);
  const mutable = { ...requests.bond, metrics: ["dv01"] };
  const options = priceOptions(client, 1, mutable);
  mutable.metrics.push("theta");
  expect(options.queryKey.at(-1).metrics).toEqual(["dv01"]);
  query.clear();
});
it("uses the exact native option registries and canonicalizes instrument text", async () => {
  expect(unwrap(await proxy.models())).toEqual(native.listModelsGrouped());
  expect(unwrap(await proxy.metrics())).toEqual(
    native.listStandardMetricsGrouped(),
  );
  expect(unwrap(await proxy.calendars())).toEqual(native.availableCalendars());
  expect(unwrap(await proxy.validate(requests.bond.instrumentJson))).toBe(
    native.validateInstrumentJson(requests.bond.instrumentJson),
  );
  expect((await proxy.validate("{")).ok).toBe(false);
});
it("keeps four native handles, refreshes recency, frees evictions until worker termination", async () => {
  const local = await startWorker();
  try {
    const markets = Array.from({ length: 5 }, (_, i) => {
      const state = JSON.parse(requests.bond.marketJson);
      state.curves.find(
        (curve) => curve.type === "discount",
      ).knot_points[1][1] -= i * 0.01;
      const handle = new native.Market(JSON.stringify(state));
      try {
        return handle.toJson();
      } finally {
        handle.free();
      }
    });
    for (const marketJson of markets.slice(0, 4))
      unwrap(await local.proxy.price({ ...requests.bond, marketJson }));
    unwrap(
      await local.proxy.price({ ...requests.bond, marketJson: markets[0] }),
    );
    unwrap(
      await local.proxy.price({ ...requests.bond, marketJson: markets[4] }),
    );
    expect(await local.proxy.resources()).toMatchObject({
      constructed: 5,
      freed: 1,
      calls: 6,
    });
    unwrap(
      await local.proxy.price({ ...requests.bond, marketJson: markets[0] }),
    );
    expect((await local.proxy.resources()).constructed).toBe(5);
    unwrap(
      await local.proxy.price({ ...requests.bond, marketJson: markets[1] }),
    );
    expect((await local.proxy.resources()).freed).toBe(2);
  } finally {
    await local.close();
  }
});
it("preserves initialization failure fields and rejects every later operation", async () => {
  const local = await startWorker({ fail: true });
  try {
    const initial = await local.proxy.initialize();
    expect(initial).toMatchObject({
      ok: false,
      error: {
        name: "TypeError",
        message: "Native initialization failed",
        kind: "init",
        report: { wasm: "missing" },
        cause: { name: "Error", message: "fixture cause" },
      },
    });
    expect(await local.proxy.price(requests.bond)).toEqual(initial);
    expect(() => unwrap(initial)).toThrow("Native initialization failed");
  } finally {
    await local.close();
  }
});

it("exports identical host values and rejects non-bigint seeds through both boundaries", async () => {
  const result = unwrap(await proxy.price(requests.stochastic));
  expect(result.details.type).toBe("monte_carlo");
  for (const seed of [result.details.data.seed, 0n, (1n << 64n) - 1n]) {
    const value = structuredClone(result);
    value.details.data.seed = seed;
    expect(unwrap(await proxy.exportResult(value))).toBe(
      exportValuation(value, native.validateValuationResultJson),
    );
  }
  for (const seed of [0, 42, Number.MAX_SAFE_INTEGER, "42", -1n, 1n << 64n]) {
    const value = structuredClone(result);
    value.details.data.seed = seed;
    expect(() =>
      exportValuation(value, native.validateValuationResultJson),
    ).toThrow();
    expect((await proxy.exportResult(value)).ok).toBe(false);
  }
});

it("retains validated canonical markets for pricing and cashflows and frees equivalent duplicates", async () => {
  const local = await startWorker();
  try {
    const canonical = unwrap(
      await local.proxy.validateMarket(requests.bond.marketJson),
    );
    const request = { ...requests.bond, marketJson: canonical };
    unwrap(await local.proxy.price(request));
    unwrap(await local.proxy.cashflows(request));
    expect(await local.proxy.resources()).toMatchObject({
      constructed: 1,
      freed: 0,
    });
    expect(unwrap(await local.proxy.validateMarket(canonical + " "))).toBe(
      canonical,
    );
    expect(await local.proxy.resources()).toMatchObject({
      constructed: 2,
      freed: 1,
    });
    expect((await local.proxy.validateMarket("{")).ok).toBe(false);
    unwrap(await local.proxy.price(request));
    expect(await local.proxy.resources()).toMatchObject({
      constructed: 2,
      freed: 1,
    });
  } finally {
    await local.close();
  }
});

it("frees failed canonicalization handles without caching them", async () => {
  const local = await startWorker({ failMarketSerialization: true });
  try {
    for (let i = 1; i <= 2; i++) {
      expect(
        await local.proxy.validateMarket(requests.bond.marketJson),
      ).toMatchObject({
        ok: false,
        error: { name: "TypeError", message: "Market serialization failed" },
      });
      expect(await local.proxy.resources()).toMatchObject({
        constructed: i,
        freed: i,
      });
    }
  } finally {
    await local.close();
  }
});
