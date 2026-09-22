import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { beforeAll, afterAll, it, expect } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { startWorker } from "../shared/worker/harness.mjs";
import {
  unwrap,
  errorValue,
  type CubeSampleRequest,
} from "@/workers/finstack-contract";
import { cubeOptions } from "@/hooks/models/use-cube-samples/use-cube-samples";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { serializeHost } from "../../src/codec.mjs";
import fixture from "../core/cubes/cases.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
}, 30000);
afterAll(async () => worker?.close());
const cubes = fixture.market.vol_cubes as CubeSampleRequest["cube"][];
const request = (
  cube = cubes[0]!,
  convention: CubeSampleRequest["convention"] = "normal",
): CubeSampleRequest => ({
  cube,
  convention,
  coordinates: [
    {
      expiry: cube.expiries[0]!,
      tenor: cube.tenors[0]!,
      strike: cube.id === "SHIFTED-BLACK" ? 0.005 : 0.05,
    },
  ],
});
function direct(r: CubeSampleRequest) {
  const handle = native.VolCube.fromJson(serializeHost(r.cube));
  try {
    return r.coordinates.map((p) =>
      (r.convention === "normal" ? native.getCubeNormalVol : native.getCubeVol)(
        handle,
        p.expiry,
        p.tenor,
        p.strike,
      ),
    );
  } finally {
    handle.free();
  }
}
const resources = () =>
  (
    worker.proxy as unknown as {
      resources(): Promise<{
        cubeConstructed: number;
        cubeFreed: number;
        cubeJson: string;
      }>;
    }
  ).resources();
it("uses the fresh native 07 calibration output, with missing shifts preserved in canonical data", () => {
  const source = readFileSync(
    fileURLToPath(new URL(`../../../${fixture.source}`, import.meta.url)),
    "utf8",
  );
  expect(createHash("sha256").update(source).digest("hex")).toBe(
    fixture.sha256,
  );
  const actual = native.calibrate(source).result.final_market.vol_cubes[0];
  expect(cubes.find((c) => c.id === actual.id)).toEqual(actual);
});
it("matches both checked evaluators for absent/present shifts and complete state/coordinate changes", async () => {
  const before = await resources();
  const variants: CubeSampleRequest[] = [];
  for (const cube of cubes)
    for (const convention of ["normal", "black_lognormal"] as const)
      variants.push(request(cube, convention));
  const normal = cubes.find((c) => c.id !== "SHIFTED-BLACK")!;
  for (const field of ["alpha", "beta", "rho", "nu", "shift"] as const) {
    const r = structuredClone(request(normal));
    r.cube.params[0]![field] = (r.cube.params[0]![field] ?? 0) + 0.001;
    variants.push(r);
  }
  const forward = structuredClone(request(normal));
  forward.cube.forwards[0]! += 0.001;
  variants.push(forward);
  variants.push({
    ...request(normal),
    cube: {
      ...normal,
      interpolation_mode:
        normal.interpolation_mode === "vol" ? "total_variance" : "vol",
    },
  });
  variants.push({
    ...request(normal),
    coordinates: [
      { expiry: normal.expiries[1]!, tenor: normal.tenors[1]!, strike: 0.051 },
    ],
  });
  const keys = variants.map((r) =>
    JSON.stringify(cubeOptions(null, 1, r).queryKey),
  );
  expect(new Set(keys).size).toBe(keys.length);
  for (const r of variants)
    expect(unwrap(await worker.proxy.sampleCube(r))).toEqual(direct(r));
  const after = await resources();
  expect(after.cubeConstructed - before.cubeConstructed).toBe(variants.length);
  expect(after.cubeFreed - before.cubeFreed).toBe(variants.length);
  await worker.proxy.sampleCube(request(normal));
  expect(JSON.parse((await resources()).cubeJson).params).toEqual(
    normal.params,
  );
  await worker.proxy.sampleCube(
    request(cubes.find((c) => c.id === "SHIFTED-BLACK")!),
  );
  expect(JSON.parse((await resources()).cubeJson).params[0].shift).toBe(0.03);
});
it("preserves checked out-of-grid/model errors and frees successfully constructed handles", async () => {
  const good = request();
  const before = await resources();
  const variants = [
    { ...good, coordinates: [{ expiry: 999, tenor: 5, strike: 0.05 }] },
    { ...good, coordinates: [{ expiry: 1, tenor: 999, strike: 0.05 }] },
    { ...good, coordinates: [{ expiry: 1, tenor: 2, strike: NaN }] },
  ];
  for (const r of variants) {
    let error;
    try {
      direct(r);
    } catch (e) {
      error = errorValue(e);
    }
    expect(error).toBeTruthy();
    expect(await worker.proxy.sampleCube(r)).toEqual({ ok: false, error });
  }
  const after = await resources();
  expect(after.cubeConstructed - before.cubeConstructed).toBe(3);
  expect(after.cubeFreed - before.cubeFreed).toBe(3);
  const invalid = structuredClone(good);
  invalid.cube.params[0]!.alpha = -1;
  let error;
  try {
    direct(invalid);
  } catch (e) {
    error = errorValue(e);
  }
  expect(await worker.proxy.sampleCube(invalid)).toEqual({ ok: false, error });
});
it("snapshots all canonical inputs and keys convention, axes, forwards and non-finite coordinates", async () => {
  const seen: CubeSampleRequest[] = [];
  const client = {
    call: async (_method: unknown, r: CubeSampleRequest) => {
      seen.push(r);
      return unwrap(await worker.proxy.sampleCube(r));
    },
  } as unknown as FinstackClient;
  const input = structuredClone(request()),
    options = cubeOptions(client, 1, input);
  input.cube.forwards[0]! += 0.01;
  input.coordinates[0]!.strike += 0.001;
  const cache = new QueryClient();
  try {
    expect(await cache.fetchQuery(options)).toEqual(direct(request()));
    expect(seen[0]).toEqual(request());
    await cache.fetchQuery(cubeOptions(client, 1, input));
    expect(seen).toHaveLength(2);
  } finally {
    cache.clear();
  }
  expect(cubeOptions(client, 1, null).enabled).toBe(false);
  const keys = [NaN, Infinity, -Infinity].map((strike) =>
    JSON.stringify(
      cubeOptions(client, 1, {
        ...request(),
        coordinates: [{ expiry: 1, tenor: 2, strike }],
      }).queryKey,
    ),
  );
  expect(new Set(keys).size).toBe(3);
  for (const field of ["expiries", "tenors", "forwards"] as const) {
    const next = structuredClone(request());
    next.cube[field][0]! += 0.001;
    expect(cubeOptions(client, 1, next).queryKey).not.toEqual(options.queryKey);
  }
  expect(cubeOptions(client, 2, request()).queryKey).not.toEqual(
    options.queryKey,
  );
});
