import { createRequire } from "node:module";
import { beforeAll, afterAll, it, expect } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { startWorker } from "../shared/worker/harness.mjs";
import {
  unwrap,
  errorValue,
  type CubeSliceRequest,
  type CubeSmileRequest,
} from "@/workers/finstack-contract";
import { cubeSliceOptions } from "@/hooks/models/use-cube-slice/use-cube-slice";
import { cubeSmileOptions } from "@/hooks/models/use-cube-smile/use-cube-smile";
import { surfaceNodes } from "@/components/finstack/core/components/vol-surface-chart/stored";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { serializeHost } from "../../src/codec.mjs";
import fixture from "../core/cubes/cases.json";
import { budget } from "../shared/budget";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
type Resources = Record<
  | "cubeConstructed"
  | "cubeFreed"
  | "sliceConstructed"
  | "sliceFreed"
  | "sabrConstructed"
  | "sabrFreed"
  | "smileConstructed"
  | "smileFreed",
  number
>;
let worker: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (worker.proxy as unknown as { resources(): Promise<Resources> }).resources();
const delta = (before: Resources, after: Resources) =>
  Object.fromEntries(
    Object.keys(before).map((k) => [
      k,
      after[k as keyof Resources] - before[k as keyof Resources],
    ]),
  );
beforeAll(async () => {
  worker = await startWorker();
});
afterAll(async () => worker?.close());
const cubes = fixture.market.vol_cubes as CubeSliceRequest["cube"][];
const normal = cubes.find((c) => c.id === "USD-SWAPTION-NORMAL-VOL")!;
const shifted = cubes.find((c) => c.id === "SHIFTED-BLACK")!;
const strikesFor = (cube: CubeSliceRequest["cube"]) =>
  cube === shifted
    ? [-0.01, -0.005, 0, 0.005, 0.01, 0.02]
    : [0.03, 0.04, 0.05, 0.06, 0.07];
const slice = (
  cube: CubeSliceRequest["cube"],
  axis: CubeSliceRequest["axis"],
  convention: CubeSliceRequest["convention"],
  at = axis === "tenor" ? cube.tenors[0]! : cube.expiries[0]!,
): CubeSliceRequest => ({
  cube,
  axis,
  at,
  strikes: strikesFor(cube),
  convention,
});
const smile = (
  cube = normal,
  extra: Partial<CubeSmileRequest> = {},
): CubeSmileRequest => ({
  cube,
  expiryIndex: 1,
  tenorIndex: 2,
  strikes: strikesFor(cube),
  rate: 0.04,
  ...extra,
});
const materializer = {
  tenor: "materializeCubeTenorSlice",
  expiry: "materializeCubeExpirySlice",
} as const;
function directSlice(r: CubeSliceRequest) {
  const cube = native.VolCube.fromJson(serializeHost(r.cube));
  try {
    const handle = native[materializer[r.axis]](
      cube,
      r.at,
      [...r.strikes],
      r.convention,
    );
    try {
      return JSON.parse(handle.toJson());
    } finally {
      handle.free();
    }
  } finally {
    cube.free();
  }
}
function directSmile(r: CubeSmileRequest) {
  const cube = native.VolCube.fromJson(serializeHost(r.cube));
  try {
    const parameters = cube.paramsAt(r.expiryIndex, r.tenorIndex);
    const forward = cube.forwardAt(r.expiryIndex, r.tenorIndex);
    const expiry = r.cube.expiries[r.expiryIndex]!;
    const params = native.SabrParameters.fromJson(parameters);
    try {
      const s = new native.SabrSmile(params, forward, expiry);
      try {
        const strikes = [...r.strikes];
        return {
          parameters,
          forward,
          expiry,
          vols: Array.from(s.generateSmile(strikes)),
          arbitrage: s.validateNoArbitrage(strikes, r.rate),
          repaired:
            r.repairIterations == null
              ? null
              : Array.from(
                  s.repairArbitrage(strikes, r.rate, r.repairIterations),
                ),
        };
      } finally {
        s.free();
      }
    } finally {
      params.free();
    }
  } finally {
    cube.free();
  }
}
it("returns every native slice for both cubes, axes and conventions and frees both handles", async () => {
  const variants: CubeSliceRequest[] = [];
  for (const cube of cubes)
    for (const axis of ["tenor", "expiry"] as const)
      for (const convention of ["normal", "lognormal"] as const)
        variants.push(slice(cube, axis, convention));
  // Off-node fixed values are interpolated by Rust.
  variants.push(slice(normal, "tenor", "normal", 3));
  variants.push(slice(normal, "expiry", "lognormal", 1.5));
  const strikes = slice(normal, "tenor", "normal");
  variants.push({ ...strikes, strikes: [0.035, 0.05, 0.065] });
  const keys = variants.map((r) =>
    JSON.stringify(cubeSliceOptions(null, 1, r).queryKey),
  );
  expect(new Set(keys).size).toBe(keys.length);
  const before = await resources();
  for (const r of variants) {
    const result = unwrap(await worker.proxy.cubeSlice(r));
    const json = directSlice(r);
    if (result.axis === "tenor") {
      expect(result.surface).toEqual(json);
      // The tenor slice is the canonical stored surface the surface views read.
      expect(surfaceNodes(result.surface)).toHaveLength(
        r.cube.expiries.length * r.strikes.length,
      );
    } else {
      expect(result.slice).toEqual(json);
      expect(result.slice.vols_row_major).toHaveLength(
        r.cube.tenors.length * r.strikes.length,
      );
    }
    expect(result.axis).toBe(r.axis);
  }
  expect(delta(before, await resources())).toMatchObject({
    cubeConstructed: variants.length,
    cubeFreed: variants.length,
    sliceConstructed: variants.length,
    sliceFreed: variants.length,
  });
});
it("retains native slice failures and frees the cube handle", async () => {
  const good = slice(normal, "tenor", "lognormal");
  const invalid: CubeSliceRequest[] = [
    { ...good, strikes: [0.05, 0.04] },
    { ...good, axis: "expiry", strikes: [0.04, 0.04] },
    // Below the 0.03 displacement the shifted SABR domain ends.
    { ...slice(shifted, "tenor", "lognormal"), strikes: [-0.05, 0] },
  ];
  const before = await resources();
  for (const r of invalid) {
    let error;
    try {
      directSlice(r);
    } catch (e) {
      error = errorValue(e);
    }
    expect(error).toBeTruthy();
    expect(await worker.proxy.cubeSlice(r)).toEqual({ ok: false, error });
  }
  expect(delta(before, await resources())).toMatchObject({
    cubeConstructed: invalid.length,
    cubeFreed: invalid.length,
    sliceConstructed: 0,
  });
});
it("returns the native node smile, arbitrage report and optional repair and frees all three handles", async () => {
  const variants: CubeSmileRequest[] = [
    smile(),
    smile(normal, { repairIterations: 10 }),
    smile(normal, { repairIterations: 0 }),
    smile(normal, { rate: 0.01 }),
    smile(normal, { expiryIndex: 0, tenorIndex: 0 }),
    smile(normal, {
      strikes: [0.001, 0.01, 0.02, 0.04, 0.05, 0.08, 0.12, 0.2],
      repairIterations: 10,
    }),
    smile(shifted, { expiryIndex: 0, tenorIndex: 0, repairIterations: 5 }),
  ];
  const keys = variants.map((r) =>
    JSON.stringify(cubeSmileOptions(null, 1, r).queryKey),
  );
  expect(new Set(keys).size).toBe(keys.length);
  const before = await resources();
  for (const r of variants) {
    const result = unwrap(await worker.proxy.cubeSmile(r));
    expect(result).toEqual(directSmile(r));
    expect(result.repaired === null).toBe(r.repairIterations == null);
  }
  expect(delta(before, await resources())).toMatchObject({
    cubeConstructed: variants.length,
    cubeFreed: variants.length,
    sabrConstructed: variants.length,
    sabrFreed: variants.length,
    smileConstructed: variants.length,
    smileFreed: variants.length,
  });
  // On a node the smile is the node's own column of the materialized slice in
  // the SABR convention (normal for this beta = 0 cube).
  const node = unwrap(
    await worker.proxy.cubeSlice({
      ...slice(normal, "expiry", "normal", normal.expiries[1]),
    }),
  );
  const result = unwrap(await worker.proxy.cubeSmile(smile()));
  if (node.axis !== "expiry") throw new Error("expected an expiry slice");
  const n = node.slice.strikes.length;
  expect(result.vols).toEqual(node.slice.vols_row_major.slice(2 * n, 3 * n));
});
it("retains native smile failures and frees every constructed handle", async () => {
  const cases: [CubeSmileRequest, Partial<Resources>][] = [
    [
      smile(normal, { expiryIndex: 9 }),
      { cubeConstructed: 1, cubeFreed: 1, sabrConstructed: 0 },
    ],
    [
      smile(normal, { rate: NaN }),
      { cubeFreed: 1, sabrFreed: 1, smileConstructed: 1, smileFreed: 1 },
    ],
    [
      smile(normal, { strikes: [0.05, 0.04] }),
      { cubeFreed: 1, sabrFreed: 1, smileConstructed: 1, smileFreed: 1 },
    ],
    [
      smile(normal, { repairIterations: 1.5 }),
      { cubeFreed: 1, sabrFreed: 1, smileConstructed: 1, smileFreed: 1 },
    ],
  ];
  for (const [r, freed] of cases) {
    let error;
    try {
      directSmile(r);
    } catch (e) {
      error = errorValue(e);
    }
    expect(error).toBeTruthy();
    const before = await resources();
    expect(await worker.proxy.cubeSmile(r)).toEqual({ ok: false, error });
    expect(delta(before, await resources())).toMatchObject(freed);
  }
});
it("snapshots the complete requests and disables missing ones", async () => {
  const seen: unknown[] = [];
  const client = {
    call: async (method: "cubeSlice" | "cubeSmile", r: never) => {
      seen.push(r);
      return unwrap<unknown>(
        method === "cubeSlice"
          ? await worker.proxy.cubeSlice(r)
          : await worker.proxy.cubeSmile(r),
      );
    },
  } as unknown as FinstackClient;
  const sliceRequest = slice(normal, "tenor", "normal"),
    smileRequest = smile();
  const mutableSlice = structuredClone(sliceRequest),
    mutableSmile = structuredClone(smileRequest);
  const sliceQuery = cubeSliceOptions(client, 1, mutableSlice),
    smileQuery = cubeSmileOptions(client, 1, mutableSmile);
  mutableSlice.cube.params[0]!.alpha += 0.001;
  (mutableSlice.strikes as number[])[0] = 0.031;
  (mutableSmile as { rate: number }).rate = 0.05;
  const cache = new QueryClient();
  try {
    expect(await cache.fetchQuery(sliceQuery)).toEqual({
      axis: "tenor",
      surface: directSlice(sliceRequest),
    });
    expect(await cache.fetchQuery(smileQuery)).toEqual(
      directSmile(smileRequest),
    );
    expect(seen).toEqual([sliceRequest, smileRequest]);
  } finally {
    cache.clear();
  }
  expect(cubeSliceOptions(client, 1, null).enabled).toBe(false);
  expect(cubeSmileOptions(client, 1, null).enabled).toBe(false);
  expect(cubeSliceOptions(client, 2, sliceRequest).queryKey).not.toEqual(
    sliceQuery.queryKey,
  );
  expect(cubeSmileOptions(client, 2, smileRequest).queryKey).not.toEqual(
    smileQuery.queryKey,
  );
});
