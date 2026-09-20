import { beforeAll, afterAll, it, expect } from "vitest";
import { mkdtemp, rm } from "node:fs/promises";
import { Worker } from "node:worker_threads";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import { wrap, releaseProxy } from "comlink";
import nodeEndpoint from "comlink/dist/esm/node-adapter.mjs";
import { QueryClient } from "@tanstack/react-query";
import { pricingCases } from "./browser/cases.mjs";
import { priceOptions } from "../registry/hooks/use-price-instrument/use-price-instrument";
import { errorValue, unwrap } from "../registry/workers/finstack-contract";
const root = fileURLToPath(new URL("../", import.meta.url));
const packagePath = path.resolve(
  root,
  "../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const native = createRequire(import.meta.url)(packagePath);
let directory, proxy, worker;
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
function start(fail = false) {
  const worker = new Worker(path.join(directory, "node.mjs"), {
    workerData: { packagePath, fail },
  });
  return { worker, proxy: wrap(nodeEndpoint(worker)) };
}
beforeAll(async () => {
  directory = await mkdtemp(path.join(root, ".worker-test-"));
  await build({
    root,
    configFile: false,
    logLevel: "error",
    resolve: { alias: { "@/lib/finstack": path.join(root, "src") } },
    build: {
      ssr: true,
      target: "node24",
      outDir: directory,
      emptyOutDir: false,
      rolldownOptions: { output: { entryFileNames: "node.mjs" } },
      lib: {
        entry: path.join(root, "tests/worker/node.mjs"),
        formats: ["es"],
        fileName: () => "node.mjs",
      },
    },
  });
  ({ worker, proxy } = start());
  expect(
    await Promise.race([
      proxy.initialize(),
      new Promise((_, reject) => worker.once("error", reject)),
    ]),
  ).toEqual({
    ok: true,
    value: { state: "ready", worker: true },
  });
}, 30000);
afterAll(async () => {
  proxy?.[releaseProxy]();
  await worker?.terminate();
  await rm(directory, { recursive: true, force: true });
});
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
it("uses the exact native option registries and tags validation with its revision", async () => {
  expect(unwrap(await proxy.models())).toEqual(native.listModelsGrouped());
  expect(unwrap(await proxy.metrics())).toEqual(
    native.listStandardMetricsGrouped(),
  );
  expect(unwrap(await proxy.calendars())).toEqual(native.availableCalendars());
  expect(
    unwrap(
      await proxy.validate({
        instrumentJson: requests.bond.instrumentJson,
        revision: 7,
      }),
    ),
  ).toEqual({
    revision: 7,
    json: native.validateInstrumentJson(requests.bond.instrumentJson),
  });
  expect((await proxy.validate({ instrumentJson: "{", revision: 8 })).ok).toBe(
    false,
  );
});
it("keeps four native handles, refreshes recency, frees eviction and disposal", async () => {
  const local = start();
  try {
    const markets = Array.from(
      { length: 5 },
      (_, i) => requests.bond.marketJson + " ".repeat(i),
    );
    for (const marketJson of markets.slice(0, 4))
      unwrap(await local.proxy.price({ ...requests.bond, marketJson }));
    unwrap(
      await local.proxy.price({ ...requests.bond, marketJson: markets[0] }),
    );
    unwrap(
      await local.proxy.price({ ...requests.bond, marketJson: markets[4] }),
    );
    expect(await local.proxy.resources()).toEqual({
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
    unwrap(await local.proxy.dispose());
    const resources = await local.proxy.resources();
    expect(resources.freed).toBe(resources.constructed);
    expect((await local.proxy.price(requests.bond)).ok).toBe(false);
  } finally {
    local.proxy[releaseProxy]();
    await local.worker.terminate();
  }
});
it("preserves initialization failure fields and rejects every later operation", async () => {
  const local = start(true);
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
    local.proxy[releaseProxy]();
    await local.worker.terminate();
  }
});
