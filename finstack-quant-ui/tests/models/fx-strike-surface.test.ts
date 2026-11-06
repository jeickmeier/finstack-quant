import { createRequire } from "node:module";
import { beforeAll, afterAll, it, expect } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { startWorker } from "../shared/worker/harness.mjs";
import {
  unwrap,
  errorValue,
  type FxStrikeSurfaceRequest,
} from "@/workers/finstack-contract";
import { fxStrikeSurfaceOptions } from "@/hooks/models/use-fx-strike-surface/use-fx-strike-surface";
import { surfaceNodes } from "@/components/finstack/core/components/vol-surface-chart/stored";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { serializeHost } from "../../src/codec.mjs";
import { fxQuotes } from "../core/surfaces/fixtures";
import { budget } from "../shared/budget";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
type Resources = {
  fxConstructed: number;
  fxFreed: number;
  gridConstructed: number;
  gridFreed: number;
};
let worker: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (worker.proxy as unknown as { resources(): Promise<Resources> }).resources();
beforeAll(async () => {
  worker = await startWorker();
});
afterAll(async () => worker?.close());
const request: FxStrikeSurfaceRequest = {
  surface: fxQuotes,
  spot: 1.1,
  domesticRate: 0.04,
  foreignRate: 0.03,
};
function direct(request: FxStrikeSurfaceRequest) {
  const surface = native.FxDeltaVolSurface.fromJson(
    serializeHost(request.surface),
  );
  try {
    const grid = native.materializeFxDeltaSurface(
      surface,
      request.spot,
      request.domesticRate,
      request.foreignRate,
    );
    try {
      return JSON.parse(grid.toJson());
    } finally {
      grid.free();
    }
  } finally {
    surface.free();
  }
}
it("returns the native strike-space surface JSON after every quote, spot and rate change and frees both handles", async () => {
  const variants: FxStrikeSurfaceRequest[] = [request];
  for (const key of ["atm_vols", "rr_25d", "bf_25d"] as const) {
    const r = structuredClone(request);
    r.surface[key][1] += 0.001;
    variants.push(r);
  }
  for (const key of ["spot", "domesticRate", "foreignRate"] as const)
    variants.push({ ...request, [key]: request[key] + 0.01 });
  variants.push({
    ...request,
    surface: { ...fxQuotes, rr_10d: [0.02, 0.025], bf_10d: [0.008, 0.009] },
  });
  expect(
    new Set(
      variants.map((variant) =>
        JSON.stringify(fxStrikeSurfaceOptions(null, 1, variant).queryKey),
      ),
    ).size,
  ).toBe(variants.length);
  const before = await resources();
  for (const variant of variants) {
    const grid = unwrap(await worker.proxy.materializeFxSurface(variant));
    expect(grid).toEqual(direct(variant));
    // The returned JSON is the canonical stored grid the surface views read.
    expect(grid.secondary_axis).toBe("strike");
    expect(surfaceNodes(grid)).toHaveLength(
      grid.expiries.length * grid.strikes.length,
    );
  }
  const after = await resources();
  expect(after.fxConstructed - before.fxConstructed).toBe(variants.length);
  expect(after.fxFreed - before.fxFreed).toBe(variants.length);
  expect(after.gridConstructed - before.gridConstructed).toBe(variants.length);
  expect(after.gridFreed - before.gridFreed).toBe(variants.length);
});
it("retains native constructor and materialization failures and frees the quote handle", async () => {
  const before = await resources();
  const invalid: FxStrikeSurfaceRequest[] = [
    { ...request, surface: { ...fxQuotes, rr_10d: [0.02, 0.025] } },
    { ...request, spot: -1 },
    { ...request, spot: 0 },
    { ...request, domesticRate: NaN },
  ];
  for (const r of invalid) {
    let error;
    try {
      direct(r);
    } catch (e) {
      error = errorValue(e);
    }
    expect(error).toBeTruthy();
    expect(await worker.proxy.materializeFxSurface(r)).toEqual({
      ok: false,
      error,
    });
  }
  const after = await resources();
  // The constructor rejects the unpaired 10-delta wing before a handle exists;
  // the three spot/rate failures happen in materialization, after construction,
  // so no strike-space handle is ever returned.
  expect(after.fxConstructed - before.fxConstructed).toBe(3);
  expect(after.fxFreed - before.fxFreed).toBe(3);
  expect(after.gridConstructed - before.gridConstructed).toBe(0);
});
it("snapshots the complete request and disables a missing one", async () => {
  const seen: FxStrikeSurfaceRequest[] = [];
  const client = {
    call: async (_method: unknown, r: FxStrikeSurfaceRequest) => {
      seen.push(r);
      return unwrap(await worker.proxy.materializeFxSurface(r));
    },
  } as unknown as FinstackClient;
  const mutable = structuredClone(request),
    options = fxStrikeSurfaceOptions(client, 1, mutable);
  mutable.surface.atm_vols[0] = 0.3;
  (mutable as { spot: number }).spot = 1.2;
  const cache = new QueryClient();
  try {
    expect(await cache.fetchQuery(options)).toEqual(direct(request));
    expect(seen[0]).toEqual(request);
  } finally {
    cache.clear();
  }
  expect(fxStrikeSurfaceOptions(client, 1, null).enabled).toBe(false);
  expect(fxStrikeSurfaceOptions(client, 2, request).queryKey).not.toEqual(
    options.queryKey,
  );
});
