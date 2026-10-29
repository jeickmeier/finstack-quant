import { createRequire } from "node:module";
import { beforeAll, afterAll, it, expect } from "vitest";
import { startWorker } from "../shared/worker/harness.mjs";
import {
  unwrap,
  errorValue,
  type CheckSurfaceRequest,
} from "@/workers/finstack-contract";
import { checkSurfaceOptions } from "@/hooks/models/use-check-surface/use-check-surface";
import { serializeHost } from "../../src/codec.mjs";
import calibration from "../calibration/cases.json";
import { budget } from "../shared/budget";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
type Resources = Record<"volSurfaceConstructed" | "volSurfaceFreed", number>;
let worker: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (worker.proxy as unknown as { resources(): Promise<Resources> }).resources();
beforeAll(async () => {
  worker = await startWorker();
}, budget(30000));
afterAll(async () => worker?.close());
/** AAPL-EQUITY-VOL as calibrated from market_bootstrap/08_equity_vol_surface.json. */
const surface = (
  calibration.find((c) => c.source.includes("08_equity"))!.result.result
    .final_market.surfaces as CheckSurfaceRequest["surface"][]
).find((s) => s.id === "AAPL-EQUITY-VOL")!;
/** The 0.5y at-the-money node raised to 0.40: a butterfly (and calendar, local-vol) violation. */
const butterfly = {
  ...surface,
  vols_row_major: surface.vols_row_major.map((v, i) => (i === 4 ? 0.4 : v)),
};
const normal = { ...surface, quote_type: "normal" as const };
function direct(r: CheckSurfaceRequest) {
  const handle = native.VolSurface.fromJson(serializeHost(r.surface));
  try {
    return native.checkSurface(handle, {
      forward_prices: [...r.forwards],
      ...(r.tolerance == null ? {} : { tolerance: r.tolerance }),
    });
  } finally {
    handle.free();
  }
}
async function delta(run: () => Promise<void>) {
  const before = await resources();
  await run();
  const after = await resources();
  return {
    constructed: after.volSurfaceConstructed - before.volSurfaceConstructed,
    freed: after.volSurfaceFreed - before.volSurfaceFreed,
  };
}
it("passes the calibrated surface with a broadcast forward and frees the handle", async () => {
  const request = { surface, forwards: [175] };
  let report;
  expect(
    await delta(async () => {
      report = unwrap(await worker.proxy.checkSurface(request));
    }),
  ).toEqual({ constructed: 1, freed: 1 });
  expect(report).toEqual(direct(request));
  expect(report).toEqual({
    vol_surface_id: "AAPL-EQUITY-VOL",
    violations: [],
    passed: true,
    counts_by_type: {},
    counts_by_severity: {},
  });
  // One forward per expiry is the same check.
  expect(
    unwrap(
      await worker.proxy.checkSurface({ surface, forwards: [175, 175, 175] }),
    ),
  ).toEqual(report);
});
it("reports an injected butterfly violation as Rust returns it, with an explicit tolerance", async () => {
  const request = { surface: butterfly, forwards: [175], tolerance: 1e-8 };
  const report = unwrap(await worker.proxy.checkSurface(request));
  expect(report).toEqual(direct(request));
  expect(report.passed).toBe(false);
  expect(report.counts_by_type.butterfly).toBe(1);
  const violation = report.violations.find(
    (v) => v.violation_type === "butterfly",
  )!;
  expect(violation.location).toMatchObject({ strike: 175, expiry: 0.50410959 });
  expect(violation.severity).toBe("critical");
  // The total count matches the returned list.
  expect(Object.values(report.counts_by_type).reduce((a, b) => a + b, 0)).toBe(
    report.violations.length,
  );
});
it("returns the native convention error for a normal-quoted surface and frees the handle", async () => {
  const request = { surface: normal, forwards: [175] };
  let error: ReturnType<typeof errorValue> | undefined;
  try {
    direct(request);
  } catch (e) {
    error = errorValue(e);
  }
  expect(error).toMatchObject({ kind: "validation" });
  expect(error!.message).toContain("normal");
  expect(
    await delta(async () => {
      expect(await worker.proxy.checkSurface(request)).toEqual({
        ok: false,
        error,
      });
    }),
  ).toEqual({ constructed: 1, freed: 1 });
});
it("leaves forward validation to Rust and keys every request distinctly", async () => {
  const wrongLength = { surface, forwards: [175, 176] };
  const result = await worker.proxy.checkSurface(wrongLength);
  expect(result).toMatchObject({ ok: false, error: { kind: "validation" } });
  const keys = [
    { surface, forwards: [175] },
    { surface, forwards: [176] },
    { surface, forwards: [175], tolerance: 1e-8 },
    { surface: butterfly, forwards: [175] },
  ].map((r) => JSON.stringify(checkSurfaceOptions(null, 1, r).queryKey));
  expect(new Set(keys).size).toBe(keys.length);
});
