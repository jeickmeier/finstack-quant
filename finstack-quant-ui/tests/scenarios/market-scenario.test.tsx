// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import { cleanup } from "@testing-library/react";
import {
  marketScenarioOptions,
  type MarketScenarioRequest,
} from "@/hooks/scenarios/use-market-scenario/use-market-scenario";
import { applyMarketScenarioOptions } from "@/hooks/scenarios/use-apply-market-scenario/use-apply-market-scenario";
import {
  unwrap,
  type MarketScenarioApplicationRequest,
} from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./application/cases.json";
import bond from "../../src/fixtures/results/bond.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
let harness: Awaited<ReturnType<typeof startWorker>>;
let call: ReturnType<typeof vi.fn>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, request: unknown) => {
    if (method === "initialize")
      return unwrap(await harness.proxy.initialize());
    if (method === "buildMarketScenario")
      return unwrap(
        await harness.proxy.buildMarketScenario(
          request as MarketScenarioRequest,
        ),
      );
    throw new Error(`Unexpected worker call ${method}`);
  });
  mock.createClient.mockReturnValue({ call, close() {} });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
/** The fixture's operations draft: one curve id per shift and basis points as typed. */
interface MarketScenarioDraft {
  id: string;
  shifts: {
    curveKind: MarketScenarioRequest["shifts"][number]["curveKind"];
    curveId: string;
    bp: string;
  }[];
  roll: { period: string; rollMode: string } | null;
  hazardBumpMode: string;
}
const drafts = fixture.cases.map((entry) => ({
  ...entry,
  draft: entry.draft as MarketScenarioDraft,
}));
/** The builder's conversion: the draft's curve id as a one-entry list and bp as a number. */
const request = (draft: MarketScenarioDraft): MarketScenarioRequest => ({
  id: draft.id,
  shifts: draft.shifts.map((shift) => ({
    curveKind: shift.curveKind,
    curveIds: [shift.curveId],
    bp: Number(shift.bp),
  })),
  roll: draft.roll && {
    period: draft.roll.period,
    rollMode: (draft.roll.rollMode || null) as NonNullable<
      MarketScenarioRequest["roll"]
    >["rollMode"],
  },
  hazardBumpMode: (draft.hazardBumpMode ||
    null) as MarketScenarioRequest["hazardBumpMode"],
});
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
it("pins the JSON inputs the scenario fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/scenarios/application/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it.each(drafts)(
  "builds and applies $name through the real worker exactly as the native fixture",
  async (entry) => {
    const spec = unwrap(
      await harness.proxy.buildMarketScenario(request(entry.draft)),
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
  const [shift, roll] = drafts;
  const shiftSpec = JSON.parse(shift!.scenarioJson);
  expect(shiftSpec).not.toHaveProperty("hazard_bump_mode");
  expect(native.scenarioSpecRequiresInstruments(shift!.scenarioJson)).toBe(
    false,
  );
  // The roll mode was left blank; the operation carries Rust's own default.
  const rollOperation = JSON.parse(roll!.scenarioJson).operations[0];
  expect(rollOperation).toEqual(
    native.operationSpecTimeRollForward(roll!.draft.roll!.period),
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
  const blank = await harness.proxy.buildMarketScenario({
    ...request(drafts[0]!.draft),
    shifts: [{ curveKind: "discount", curveIds: ["USD-OIS"], bp: Number.NaN }],
  });
  expect(blank.ok).toBe(false);
  const hz = drafts.find((entry) => entry.name === "hz-par-cds-25bp")!;
  const solveToPar = unwrap(
    await harness.proxy.buildMarketScenario({
      ...request(hz.draft),
      hazardBumpMode: null,
    }),
  );
  const failed = await harness.proxy.applyMarketScenario(
    application(solveToPar),
  );
  expect(failed.ok).toBe(false);
  if (!failed.ok)
    expect(failed.error.message).toContain("no lossless calibration recipe");
});
it("keys both queries on the worker session and the complete snapshotted request", () => {
  const built = request(drafts[1]!.draft);
  const options = marketScenarioOptions(null, 3, built);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "buildMarketScenario",
    built,
  ]);
  expect(options.queryKey[3]).not.toBe(built);
  expect(marketScenarioOptions(null, 3, null).enabled).toBe(false);
  const applying = application(drafts[1]!.scenarioJson);
  expect(applyMarketScenarioOptions(null, 4, applying).queryKey).toEqual([
    "finstack",
    4,
    "applyMarketScenario",
    applying,
  ]);
  expect(applyMarketScenarioOptions(null, 4, null).enabled).toBe(false);
});
