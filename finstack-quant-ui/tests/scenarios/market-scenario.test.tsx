// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { useState } from "react";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  marketScenarioOptions,
  useMarketScenario,
  type MarketScenarioRequest,
} from "@/hooks/scenarios/use-market-scenario/use-market-scenario";
import { applyMarketScenarioOptions } from "@/hooks/scenarios/use-apply-market-scenario/use-apply-market-scenario";
import {
  MarketScenarioBuilder,
  type MarketScenarioDraft,
} from "@/components/finstack/scenarios/components/market-scenario-builder/market-scenario-builder";
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
const drafts = fixture.cases.map((entry) => ({
  ...entry,
  draft: entry.draft as MarketScenarioDraft,
}));
/** The panel's conversion: the draft's curve id as a one-entry list and bp as a number. */
const request = (draft: MarketScenarioDraft): MarketScenarioRequest => ({
  id: draft.id,
  shifts: draft.shifts.map((shift) => ({
    curveKind: shift.curveKind,
    curveIds: [shift.curveId],
    bp: Number(shift.bp),
  })),
  roll: draft.roll && {
    period: draft.roll.period,
    rollMode: draft.roll.rollMode || null,
  },
  hazardBumpMode: draft.hazardBumpMode || null,
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
function Controlled({
  initial,
  onChange,
}: {
  initial: MarketScenarioDraft;
  onChange: (draft: MarketScenarioDraft) => void;
}) {
  const [draft, setDraft] = useState(initial);
  return (
    <MarketScenarioBuilder
      value={draft}
      onValueChange={(next) => {
        setDraft(next);
        onChange(next);
      }}
      curveIds={["USD-OIS", "HZ"]}
    />
  );
}
it("edits shifts, the roll and the hazard bump mode without calling the worker", async () => {
  const onChange = vi.fn();
  render(
    <Controlled
      initial={{ id: "s", shifts: [], roll: null, hazardBumpMode: "" }}
      onChange={onChange}
    />,
  );
  expect(screen.getByText("No curve shifts.")).toBeTruthy();
  await userEvent.click(screen.getByRole("button", { name: "Add shift" }));
  const shift = screen.getByRole("group", { name: "Shift 1" });
  fireEvent.change(
    within(shift).getByRole("textbox", { name: "Shift 1 size" }),
    {
      target: { value: "-12.5" },
    },
  );
  fireEvent.change(
    within(shift).getByRole("combobox", { name: "Shift 1 curve id" }),
    { target: { value: "HZ" } },
  );
  await userEvent.click(
    within(shift).getByRole("combobox", { name: "Shift 1 curve kind" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "par_cds" }));
  await userEvent.click(screen.getByRole("button", { name: "Add roll" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Roll period" }), {
    target: { value: "3M" },
  });
  await userEvent.click(
    screen.getByRole("radio", { name: "first_order_shift" }),
  );
  expect(onChange).toHaveBeenLastCalledWith({
    id: "s",
    shifts: [{ curveKind: "par_cds", curveId: "HZ", bp: "-12.5" }],
    roll: { period: "3M", rollMode: "" },
    hazardBumpMode: "first_order_shift",
  });
  await userEvent.click(screen.getByRole("button", { name: "Remove" }));
  await userEvent.click(screen.getByRole("button", { name: "Remove roll" }));
  expect(onChange).toHaveBeenLastCalledWith({
    id: "s",
    shifts: [],
    roll: null,
    hazardBumpMode: "first_order_shift",
  });
  expect(call).not.toHaveBeenCalled();
});
function Connected({ initial }: { initial: MarketScenarioDraft }) {
  const [draft, setDraft] = useState(initial);
  const spec = useMarketScenario(request(draft));
  return (
    <MarketScenarioBuilder
      value={draft}
      onValueChange={setDraft}
      curveIds={["USD-OIS"]}
      specJson={spec.data}
      loading={spec.isFetching}
      error={spec.error?.message}
    />
  );
}
it("shows the Rust-built spec, then the native error for an invalid roll period", async () => {
  const entry = drafts[1]!;
  render(
    <FinstackQueryProvider>
      <Connected initial={entry.draft} />
    </FinstackQueryProvider>,
  );
  const viewer = screen.getByRole("region", { name: "Scenario spec" });
  await waitFor(
    () =>
      expect(viewer.textContent).toContain(
        JSON.parse(entry.scenarioJson).operations[0].kind,
      ),
    { timeout: budget(5000) },
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Roll period" }), {
    target: { value: "1 month" },
  });
  await waitFor(
    () => expect(viewer.textContent).toContain("not a valid tenor"),
    { timeout: budget(5000) },
  );
});
