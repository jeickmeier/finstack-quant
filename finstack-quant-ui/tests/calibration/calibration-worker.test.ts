import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, afterAll, it, expect } from "vitest";
import { QueryClient } from "@tanstack/react-query";
import { startWorker } from "../shared/worker/harness.mjs";
import { unwrap, errorValue } from "@/workers/finstack-contract";
import { calibrationOptions } from "@/hooks/calibration/use-calibrate/use-calibrate";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { serializeHost } from "../../src/codec.mjs";
import fixtures from "../../src/generated/fixtures.json";
import bond from "../../src/fixtures/results/bond.json";
import recorded from "./cases.json";
import { budget } from "../shared/budget";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const cases = fixtures
  .filter((f) => f.kind === "calibration-input")
  .map((f) => ({
    source: f.source,
    json: readFileSync(
      resolve(import.meta.dirname, "../../..", f.source),
      "utf8",
    ),
  }));
// Native validation and solve results come from tests/calibration/cases.json,
// which `mise run ui-fixtures --check` regenerates and compares in the same gate.
const nativeResult = (source: string) =>
  recorded.find((c) => c.source === source && !("variant" in c))!;
const stepKinds = (source: string) =>
  nativeResult(source).input.plan.steps.map((step) => step.kind);
// 12_full_credit_desk_market solves every step kind 05_cdx_base_correlation does,
// so 05, the costliest solve, is validated and dry-run through the worker only.
const unsolved = cases.find((c) =>
  c.source.endsWith("05_cdx_base_correlation.json"),
)!;
const solved = cases.filter((c) => c !== unsolved);
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
}, budget(30000));
afterAll(async () => worker?.close());
it("solves every calibration step kind through the worker", () => {
  const covered = new Set(solved.flatMap((c) => stepKinds(c.source)));
  expect(
    stepKinds(unsolved.source).filter((kind) => !covered.has(kind)),
  ).toEqual([]);
});
for (const fixture of cases)
  it(
    `preserves complete native validation, dry-run and solve: ${fixture.source}`,
    async () => {
      const expected = nativeResult(fixture.source);
      expect(
        unwrap(await worker.proxy.validateCalibration(fixture.json)),
      ).toEqual(expected.input);
      expect(unwrap(await worker.proxy.dryRun(fixture.json))).toEqual(
        native.dryRun(fixture.json),
      );
      if (fixture !== unsolved)
        expect(unwrap(await worker.proxy.calibrate(fixture.json))).toEqual(
          expected.result,
        );
    },
    budget(30000),
  );
it("retains every structured failure field, including absent and available solver diagnostics", async () => {
  const missing = JSON.parse(cases[0]!.json);
  missing.plan.steps[0].quote_set = "missing_quotes";
  const equity = JSON.parse(
    cases.find((c) => c.source.includes("08_equity"))!.json,
  );
  const target = structuredClone(equity);
  target.plan.settings.fail_on_bad_fit = true;
  target.plan.settings.vol_surface = { validation_tolerance: 1e-4 };
  const solver = structuredClone(equity);
  solver.plan.steps[1].target_strikes = [140, 180, 220];
  solver.plan.settings.fail_on_bad_fit = true;
  solver.plan.settings.vol_surface = { validation_tolerance: 0.001 };
  for (const json of [
    "{ malformed",
    JSON.stringify(missing),
    JSON.stringify(target),
    JSON.stringify(solver),
  ]) {
    let direct;
    try {
      native.calibrate(json);
    } catch (error) {
      direct = errorValue(error);
    }
    expect(direct).toBeTruthy();
    const result = await worker.proxy.calibrate(json);
    expect(result).toEqual({ ok: false, error: direct });
    for (const key of [
      "kind",
      "stage",
      "step_id",
      "solver_diagnostics",
      "details",
      "cause",
    ])
      expect(result.ok ? undefined : result.error[key]).toEqual(direct![key]);
  }
  const report = unwrap(await worker.proxy.dryRun(JSON.stringify(missing)));
  expect(report).toEqual(native.dryRun(JSON.stringify(missing)));
  expect(report.errors.length).toBeGreaterThan(0);
});
it(
  "keys all quotes, prior markets, settings and sessions, and hands the returned market into native pricing",
  async () => {
    const calls: string[] = [];
    const client = {
      call: async (method: "calibrate" | "dryRun", json: string) => {
        calls.push(json);
        return unwrap<unknown>(await worker.proxy[method](json));
      },
    } as unknown as FinstackClient;
    const query = new QueryClient();
    try {
      const base = JSON.parse(cases[0]!.json),
        quote = structuredClone(base),
        settings = structuredClone(base),
        prior = structuredClone(base);
      quote.market_data[0].rate = 0.0527;
      settings.plan.settings.compute_diagnostics = true;
      prior.prior_market = JSON.parse(
        cases.find((c) => c.source.includes("05_cdx_base"))!.json,
      ).prior_market;
      const editedPrior = structuredClone(prior);
      editedPrior.prior_market[0].knot_points[1][1] = 0.0013;
      const inputs = [base, quote, settings, prior, editedPrior].map((value) =>
        JSON.stringify(value),
      );
      expect(
        calibrationOptions(client, 1, "calibrate", JSON.stringify(prior))
          .queryKey,
      ).not.toEqual(
        calibrationOptions(client, 1, "calibrate", inputs[0]!).queryKey,
      );
      for (const json of inputs) {
        const result = await query.fetchQuery(
          calibrationOptions(client, 1, "calibrate", json),
        );
        expect(result).toEqual(native.calibrate(json));
      }
      await query.fetchQuery(
        calibrationOptions(client, 1, "calibrate", inputs[0]!),
      );
      expect(calls).toHaveLength(5);
      await query.fetchQuery(
        calibrationOptions(client, 2, "calibrate", inputs[0]!),
      );
      expect(calls).toHaveLength(6);
      expect(
        calibrationOptions(client, 1, "dryRun", inputs[0]!).queryKey,
      ).not.toEqual(
        calibrationOptions(client, 1, "calibrate", inputs[0]!).queryKey,
      );
      expect(calibrationOptions(client, 1, "calibrate", null).enabled).toBe(
        false,
      );
      const result = await query.fetchQuery(
        calibrationOptions(client, 1, "calibrate", inputs[0]!),
      );
      const request = {
        ...bond.request,
        marketJson: serializeHost(result.result.final_market),
      };
      const actual = unwrap(await worker.proxy.price(request));
      const expected = native.priceInstrument(
        request.instrumentJson,
        request.marketJson,
        request.asOf,
        request.model,
        request.metrics,
      );
      expected.meta.timestamp = actual.meta.timestamp;
      expect(actual).toEqual(expected);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);
