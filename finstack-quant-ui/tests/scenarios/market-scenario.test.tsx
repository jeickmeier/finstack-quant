// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { applyMarketScenarioOptions } from "@/hooks/scenarios/use-apply-market-scenario/use-apply-market-scenario";
import {
  unwrap,
  type MarketScenarioApplicationRequest,
} from "@/workers/finstack-contract";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import fixture from "./application/cases.json";
import bond from "../../src/fixtures/results/bond.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
const harness = useWorkerHarness([
  "validateScenarioSpec",
  "applyMarketScenario",
]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const cases = fixture.cases;
const base = bond.request;
const application = (
  scenarioJson: string,
): MarketScenarioApplicationRequest => ({
  scenarioJson,
  marketJson: base.marketJson,
  asOf: base.asOf,
  instrumentJson: base.instrumentJson,
});
// Applications and shocked prices are regenerated and compared by
// `mise run ui-fixtures --check` (tests/scenarios/application/generate.mjs).
it.each(cases)(
  "validates and applies $name through the real worker exactly as the native fixture",
  async (entry) => {
    const spec = unwrap(
      await harness.proxy.validateScenarioSpec(JSON.stringify(entry.spec)),
    );
    expect(spec).toBe(entry.scenarioJson);
    const applied = unwrap(
      await harness.proxy.applyMarketScenario(application(spec)),
    );
    expect(applied.report).toEqual(entry.report);
    expect(applied.asOf).toBe(entry.asOf);
    // The shocked market is canonical native text; the bond is not mutated.
    expect(applied.marketJson).toBe(
      native.MarketContext.fromJson(applied.marketJson).toJson(),
    );
    expect(applied.instrumentJson).toBe(base.instrumentJson);
  },
  budget(15000),
);
it("leaves every omitted option to Rust and sends the instrument only when the spec needs it", async () => {
  const [shift, roll] = cases;
  const shiftSpec = JSON.parse(shift!.scenarioJson);
  expect(shiftSpec).not.toHaveProperty("hazard_bump_mode");
  expect(native.scenarioSpecRequiresInstruments(shift!.scenarioJson)).toBe(
    false,
  );
  // The roll mode was omitted; the canonical operation carries Rust's own default.
  const rollOperation = JSON.parse(roll!.scenarioJson).operations[0];
  expect(rollOperation).toEqual(
    native.operationSpecTimeRollForward(
      (roll!.spec.operations[0] as { period: string }).period,
    ),
  );
  expect(native.scenarioSpecRequiresInstruments(roll!.scenarioJson)).toBe(true);
  expect(roll!.report.time_roll?.new_date).toBe("2025-02-03");
  expect(roll!.report.time_roll?.total_carry).toEqual({
    USD: { amount: "4613.745264131", currency: "USD" },
  });
  expect(roll!.shocked.value).toEqual({
    amount: "1002074.235114868",
    currency: "USD",
  });
});
it("returns the native errors for a blank size and an unsupported hazard bump", async () => {
  const blank = await harness.proxy.validateScenarioSpec(
    JSON.stringify({
      id: "blank",
      operations: [
        {
          kind: "curve_parallel_bp",
          curve_kind: "discount",
          curve_id: "USD-OIS",
          bp: null,
        },
      ],
    }),
  );
  expect(blank.ok).toBe(false);
  const hz = cases.find((entry) => entry.name === "hz-par-cds-25bp")!;
  const { hazard_bump_mode: _mode, ...solveToParSpec } = hz.spec;
  const solveToPar = unwrap(
    await harness.proxy.validateScenarioSpec(JSON.stringify(solveToParSpec)),
  );
  const failed = await harness.proxy.applyMarketScenario(
    application(solveToPar),
  );
  expect(failed.ok).toBe(false);
  if (!failed.ok)
    expect(failed.error.message).toContain("no lossless calibration recipe");
});
it("keys the application on the worker session and the complete snapshotted request", () => {
  const applying = application(cases[1]!.scenarioJson);
  expect(applyMarketScenarioOptions(null, 4, applying).queryKey).toEqual([
    "finstack",
    4,
    "applyMarketScenario",
    applying,
  ]);
  expect(applyMarketScenarioOptions(null, 4, null).enabled).toBe(false);
});
