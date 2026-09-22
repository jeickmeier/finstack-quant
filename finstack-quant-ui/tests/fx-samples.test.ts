import { createRequire } from "node:module";
import { beforeAll, afterAll, it, expect } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { startWorker } from "./worker/harness.mjs";
import {
  unwrap,
  errorValue,
  type FxDeltaSampleRequest,
} from "../registry/workers/finstack-contract";
import { fxDeltaOptions } from "../registry/hooks/use-fx-delta-samples/use-fx-delta-samples";
import type { FinstackClient } from "../registry/hooks/use-finstack/client";
import { serializeHost } from "../src/codec.mjs";
import { fxQuotes } from "./surfaces/fixtures";
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
}, 30000);
afterAll(async () => worker?.close());
const request: FxDeltaSampleRequest = {
  surface: fxQuotes,
  coordinates: [
    { expiry: 0.5, strike: 1.05, forward: 1.1 },
    { expiry: 1, strike: 1.15, forward: 1.11 },
  ],
};
function direct(request: FxDeltaSampleRequest) {
  const surface = native.FxDeltaVolSurface.fromJson(
    serializeHost(request.surface),
  );
  try {
    return request.coordinates.map((c) =>
      native.getFxDeltaVol(surface, c.expiry, c.strike, c.forward),
    );
  } finally {
    surface.free();
  }
}
it("matches direct native samples after every quote/coordinate change, including optional wings", async () => {
  const variants: FxDeltaSampleRequest[] = [request];
  for (const key of ["expiries", "atm_vols", "rr_25d", "bf_25d"] as const) {
    const r = structuredClone(request);
    r.surface[key][1] += 0.001;
    variants.push(r);
  }
  for (const key of ["expiry", "strike", "forward"] as const) {
    const r = structuredClone(request);
    r.coordinates[0]![key] += 0.01;
    variants.push(r);
  }
  variants.push({
    ...request,
    surface: { ...fxQuotes, rr_10d: [0.02, 0.025], bf_10d: [0.008, 0.009] },
  });
  expect(
    new Set(
      variants.map((variant) =>
        JSON.stringify(fxDeltaOptions(null, 1, variant).queryKey),
      ),
    ).size,
  ).toBe(variants.length);
  for (const variant of variants)
    expect(unwrap(await worker.proxy.sampleFxDelta(variant))).toEqual(
      direct(variant),
    );
  const resources = await (
    worker.proxy as unknown as {
      resources(): Promise<{ fxConstructed: number; fxFreed: number }>;
    }
  ).resources();
  expect(resources.fxConstructed).toBe(variants.length);
  expect(resources.fxFreed).toBe(variants.length);
});
it("retains native constructor/evaluator failures and frees handles after evaluator failures", async () => {
  const before = await (
    worker.proxy as unknown as {
      resources(): Promise<{ fxConstructed: number; fxFreed: number }>;
    }
  ).resources();
  const invalid = [
    { ...request, surface: { ...fxQuotes, rr_10d: [0.02, 0.025] } },
    { ...request, coordinates: [{ expiry: 1, strike: -1, forward: 1.1 }] },
    { ...request, coordinates: [{ expiry: 1, strike: 1.1, forward: NaN }] },
    { ...request, surface: { ...fxQuotes, rr_25d: [10, 10] } },
  ];
  for (const r of invalid) {
    let error;
    try {
      direct(r);
    } catch (e) {
      error = errorValue(e);
    }
    expect(error).toBeTruthy();
    expect(await worker.proxy.sampleFxDelta(r)).toEqual({ ok: false, error });
  }
  const after = await (
    worker.proxy as unknown as {
      resources(): Promise<{ fxConstructed: number; fxFreed: number }>;
    }
  ).resources();
  expect(after.fxConstructed - before.fxConstructed).toBe(3);
  expect(after.fxFreed - before.fxFreed).toBe(3);
});
it("snapshots complete request keys, disables missing requests and distinguishes non-finite native inputs", async () => {
  const seen: FxDeltaSampleRequest[] = [];
  const client = {
    call: async (_method: unknown, r: FxDeltaSampleRequest) => {
      seen.push(r);
      return unwrap(await worker.proxy.sampleFxDelta(r));
    },
  } as unknown as FinstackClient;
  const mutable = structuredClone(request),
    options = fxDeltaOptions(client, 1, mutable);
  mutable.surface.atm_vols[0] = 0.3;
  mutable.coordinates[0]!.forward = 1.2;
  const cache = new QueryClient();
  try {
    expect(await cache.fetchQuery(options)).toEqual(direct(request));
    expect(seen[0]).toEqual(request);
    await cache.fetchQuery(fxDeltaOptions(client, 1, mutable));
    expect(seen).toHaveLength(2);
  } finally {
    cache.clear();
  }
  expect(fxDeltaOptions(client, 1, null).enabled).toBe(false);
  const keys = [NaN, Infinity, -Infinity].map(
    (forward) =>
      fxDeltaOptions(client, 1, {
        ...request,
        coordinates: [{ expiry: 1, strike: 1.1, forward }],
      }).queryKey,
  );
  expect(new Set(keys.map((key) => JSON.stringify(key))).size).toBe(3);
  for (const next of [
    { ...request, surface: { ...fxQuotes, id: "changed" } },
    { ...request, coordinates: [...request.coordinates].reverse() },
  ])
    expect(fxDeltaOptions(client, 1, next).queryKey).not.toEqual(
      options.queryKey,
    );
  expect(fxDeltaOptions(client, 2, request).queryKey).not.toEqual(
    options.queryKey,
  );
});
