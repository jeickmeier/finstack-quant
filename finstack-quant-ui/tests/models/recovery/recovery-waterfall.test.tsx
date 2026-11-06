// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { useState } from "react";
import userEvent from "@testing-library/user-event";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  allocateRecoveryOptions,
  seniorityRecoveryStatsOptions,
  useAllocateRecovery,
  useSeniorityCalibrationIds,
  useSeniorityRecoveryStats,
  useWorkoutLgd,
  workoutLgdOptions,
} from "@/hooks/models/use-recovery-waterfall/use-recovery-waterfall";
import { RecoveryWaterfall } from "@/components/finstack/models/components/recovery-waterfall/recovery-waterfall";
import { SeniorityRecoveryCard } from "@/components/finstack/models/components/recovery-waterfall/seniority-recovery-card";
import { WorkoutLgdCard } from "@/components/finstack/models/components/recovery-waterfall/workout-lgd-card";
import type { OutcomeState } from "@/components/finstack/shared/primitives/native-display/native-display";
import {
  collateralTypes,
  seniorityClasses,
} from "@/components/finstack/models/components/recovery-waterfall/recovery-contracts";
import {
  unwrap,
  type Envelope,
  type RecoveryWaterfallRequest,
  type SeniorityRecoveryRequest,
  type SeniorityRecoveryStats,
  type WorkoutLgdRequest,
} from "@/workers/finstack-contract";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
import recoveryClaim from "../../../src/generated/schemas/recovery_claim.json";
import workoutLgd from "../../../src/generated/schemas/workout_lgd.json";
import { labelFor } from "../../../src/format/label.mjs";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness([
  "allocateRecovery",
  "workoutLgd",
  "seniorityRecoveryStats",
  "seniorityCalibrationIds",
]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
type NativeOutcome =
  { ok: true; value: unknown } | { ok: false; error: { message: string } };
const expectOutcome = (actual: Envelope<unknown>, result: unknown) => {
  const expected = result as NativeOutcome;
  if (expected.ok) expect(unwrap(actual)).toEqual(expected.value);
  else {
    expect(actual.ok).toBe(false);
    if (!actual.ok) expect(actual.error.message).toBe(expected.error.message);
  }
};
const value = <T,>(entry: { result: unknown }) =>
  (entry.result as { ok: true; value: T }).value;
const waterfallTerms = fixture.waterfall.shortfall
  .request as RecoveryWaterfallRequest;
const workoutTerms = fixture.workout.secured
  .request as unknown as WorkoutLgdRequest;
const statsTerms = fixture.stats.seniorUnsecured
  .request as SeniorityRecoveryRequest;
const shortfall = value<modelsWire.RecoveryWaterfallResult>(
  fixture.waterfall.shortfall,
);
/** The item's default display: four decimals, every digit in the title. */
const shown = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
interface ViewProps {
  onWaterfallSubmit?: (terms: RecoveryWaterfallRequest) => void;
  onWorkoutSubmit?: (terms: WorkoutLgdRequest) => void;
  onStatsSubmit?: (request: SeniorityRecoveryRequest) => void;
  waterfall?: OutcomeState<modelsWire.RecoveryWaterfallResult>;
  workout?: OutcomeState<modelsWire.WorkoutLgdResult>;
  stats?: OutcomeState<SeniorityRecoveryStats>;
  calibrationIds?: readonly string[] | null;
}
/** The item's three cards, each fed its own analysis. */
function Cards({
  onWaterfallSubmit = () => {},
  onWorkoutSubmit = () => {},
  onStatsSubmit = () => {},
  waterfall = { result: null },
  workout = { result: null },
  stats = { result: null },
  calibrationIds = fixture.calibrationIds,
}: ViewProps) {
  return (
    <>
      <RecoveryWaterfall
        defaultTerms={waterfallTerms}
        onSubmit={onWaterfallSubmit}
        state={waterfall}
      />
      <WorkoutLgdCard
        defaultTerms={workoutTerms}
        onSubmit={onWorkoutSubmit}
        state={workout}
      />
      <SeniorityRecoveryCard
        defaultSeniority={statsTerms.seniority}
        calibrationIds={calibrationIds}
        onSubmit={onStatsSubmit}
        state={stats}
      />
    </>
  );
}
function view(props: ViewProps) {
  return render(<Cards {...props} />);
}
it.each(Object.entries(fixture.waterfall))(
  "the worker returns the native waterfall of %s",
  async (_name, entry) => {
    expectOutcome(
      await harness.proxy.allocateRecovery(
        entry.request as RecoveryWaterfallRequest,
      ),
      entry.result,
    );
  },
);
it.each(Object.entries(fixture.workout))(
  "the worker returns the native workout LGD of %s",
  async (_name, entry) => {
    expectOutcome(
      await harness.proxy.workoutLgd(
        entry.request as unknown as WorkoutLgdRequest,
      ),
      entry.result,
    );
  },
);
it.each(Object.entries(fixture.stats))(
  "the worker returns the native seniority statistics of %s",
  async (_name, entry) => {
    expectOutcome(
      await harness.proxy.seniorityRecoveryStats(
        entry.request as SeniorityRecoveryRequest,
      ),
      entry.result,
    );
  },
);
it("the worker lists Rust's calibration ids, each one accepted", async () => {
  const ids = unwrap(await harness.proxy.seniorityCalibrationIds());
  expect(ids).toEqual(fixture.calibrationIds);
  for (const calibrationId of ids)
    expect(
      (
        await harness.proxy.seniorityRecoveryStats({
          seniority: "senior_secured",
          calibrationId,
        })
      ).ok,
    ).toBe(true);
});
it("evaluates the request's WorkoutLgd at the request's EAD", async () => {
  // Doubling the EAD with the same collateral leaves less recovered per unit.
  const larger = unwrap(
    await harness.proxy.workoutLgd({ ...workoutTerms, ead: 2_000_000 }),
  );
  const base = value<modelsWire.WorkoutLgdResult>(fixture.workout.secured);
  expect(larger.lgd).toBeGreaterThan(base.lgd);
  // A longer workout discounts the same recovery further.
  const slower = unwrap(
    await harness.proxy.workoutLgd({
      ...workoutTerms,
      model: { ...workoutTerms.model, workout_years: 5 },
    }),
  );
  expect(slower.net_recovery).toBeLessThan(base.net_recovery);
});
it("draws Rust's allocations in its order with each claim's own fields", () => {
  view({ waterfall: { result: shortfall } });
  const table = screen.getByRole("table", { name: "Recovery by claim" });
  const rows = [...table.querySelectorAll("tbody tr")];
  expect(rows.map((row) => row.children[0].textContent)).toEqual(
    shortfall.allocations.map((allocation) => allocation.id),
  );
  // The pari passu notes share a shortfall: their rate is Rust's, read here.
  const notes = shortfall.allocations[1];
  expect(notes.recovery_rate).toBeGreaterThan(0);
  expect(notes.recovery_rate).toBeLessThan(1);
  const header = [...table.querySelectorAll("thead th")].map(
    (cell) => cell.textContent,
  );
  const rate = rows[1].children[header.indexOf("Recovery rate")];
  expect(rate.textContent).toBe(shown(notes.recovery_rate));
  expect(rate.firstElementChild!.getAttribute("title")).toBe(
    String(notes.recovery_rate),
  );
  const estate = screen.getByRole("table", { name: "Estate" });
  expect(
    within(estate)
      .getByRole("rowheader", { name: "Undistributed estate" })
      .closest("tr")!.children[1].textContent,
  ).toBe(shown(shortfall.undistributed_estate));
});
it("shows the rate Rust returned even when it is not recovered over claim", () => {
  // A doctored rate: the view must show it, not recompute it.
  const doctored = {
    ...shortfall,
    allocations: shortfall.allocations.map((allocation) => ({
      ...allocation,
      recovery_rate: 0.123,
    })),
  };
  view({ waterfall: { result: doctored } });
  expect(screen.getAllByText("0.123")).toHaveLength(
    shortfall.allocations.length,
  );
});
it("offers the collateral types and seniority classes of the generated contracts and Rust's calibration ids", () => {
  view({});
  const collateral = collateralTypes.map((option) => option.value);
  const classes = seniorityClasses.map((option) => option.value);
  // Rust's declaration order, every variant.
  expect(collateral).toEqual([
    "cash",
    "securities",
    "receivables",
    "inventory",
    "equipment",
    "real_estate",
    "intellectual_property",
    "other",
  ]);
  expect(classes).toEqual([
    "1st_lien_secured",
    "2nd_lien_secured",
    "senior_secured",
    "senior_unsecured",
    "subordinated",
    "junior_subordinated",
  ]);
  const form = screen.getByRole("form", { name: "Seniority and calibration" });
  // The calibration starts at Rust's first (default) id.
  expect(within(form).getAllByRole("combobox")[1].textContent).toContain(
    "Moodys recovery 1982 2023",
  );
});
it("submits typed claims; an empty amount is NaN and an empty collateral value is unsecured", () => {
  const onWaterfallSubmit = vi.fn();
  view({ onWaterfallSubmit });
  fireEvent.click(screen.getByRole("button", { name: "Allocate" }));
  expect(onWaterfallSubmit).toHaveBeenLastCalledWith(waterfallTerms);
  const loan = screen.getByRole("group", { name: "Claim TL-B" });
  fireEvent.change(within(loan).getByLabelText("Collateral value"), {
    target: { value: "" },
  });
  fireEvent.change(within(loan).getByLabelText("Principal"), {
    target: { value: "" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Allocate" }));
  const [first] = onWaterfallSubmit.mock.lastCall![0].claims;
  expect(first.collateral_value).toBeNull();
  expect(first.principal).toBeNaN();
});
it("adds and removes claims and collateral pieces", () => {
  const onWaterfallSubmit = vi.fn();
  const onWorkoutSubmit = vi.fn();
  view({ onWaterfallSubmit, onWorkoutSubmit });
  fireEvent.click(
    within(screen.getByRole("group", { name: "Claim Sub-Notes" })).getByRole(
      "button",
      { name: "Remove claim" },
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add claim" }));
  fireEvent.click(screen.getByRole("button", { name: "Allocate" }));
  const claims = onWaterfallSubmit.mock.lastCall![0].claims;
  expect(claims.map((claim: { id: string }) => claim.id)).toEqual([
    "TL-B",
    "Notes-A",
    "Notes-B",
    "",
  ]);
  // A new claim starts empty: every amount, haircut included, is NaN until typed, never 0.
  for (const field of [
    "principal",
    "accrued",
    "penalties",
    "collateral_haircut",
  ])
    expect(claims[3][field]).toBeNaN();
  expect(claims[3].collateral_value).toBeNull();
  fireEvent.click(
    within(screen.getByRole("group", { name: "Collateral 0" })).getByRole(
      "button",
      { name: "Remove collateral" },
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "Calculate LGD" }));
  expect(onWorkoutSubmit.mock.lastCall![0].model.collateral).toEqual([
    workoutTerms.model.collateral[1],
  ]);
});
it("labels each field by its wire name and describes it from the generated contracts", async () => {
  const user = userEvent.setup();
  view({});
  const help = async (scope: HTMLElement, label: string, text: string) => {
    expect(within(scope).getByLabelText(label)).toBeTruthy();
    await user.click(
      within(scope).getByRole("button", { name: `${label} help` }),
    );
    expect((await screen.findByText(text)).textContent).toBe(text);
    await user.keyboard("{Escape}");
  };
  const loan = screen.getByRole("group", { name: "Claim TL-B" });
  for (const [field, node] of Object.entries(recoveryClaim.properties))
    await help(loan, labelFor(field), node.description);
  const workout = screen.getByRole("form", { name: "Workout terms" });
  await help(
    workout,
    "Workout years",
    workoutLgd.properties.workout_years.description,
  );
  await help(
    workout,
    "Discount rate",
    workoutLgd.properties.discount_rate.description,
  );
  expect(within(workout).getByLabelText("EAD")).toBeTruthy();
  expect(within(workout).getByLabelText("Direct cost rate")).toBeTruthy();
  expect(within(workout).getByLabelText("Indirect cost rate")).toBeTruthy();
});
it("tables the workout and seniority results as returned", () => {
  const workout = value<modelsWire.WorkoutLgdResult>(fixture.workout.secured);
  const stats = value<SeniorityRecoveryStats>(fixture.stats.seniorUnsecured);
  view({ workout: { result: workout }, stats: { result: stats } });
  const lgd = screen.getByRole("table", { name: "Workout" });
  expect(
    within(lgd).getByRole("rowheader", { name: "LGD" }).closest("tr")!
      .children[1].textContent,
  ).toBe(shown(workout.lgd));
  const beta = screen.getByRole("table", { name: "Beta recovery" });
  expect(
    within(beta).getByRole("rowheader", { name: "Mean LGD" }).closest("tr")!
      .children[1].textContent,
  ).toBe(shown(stats.mean_lgd));
});
it("shows each native error and keeps the other analyses", () => {
  const entry = fixture.stats.classNotCalibrated;
  expect(entry.result.ok).toBe(false);
  view({
    waterfall: { result: shortfall },
    stats: { result: null, error: entry.result.error!.message },
  });
  expect(screen.getByRole("alert").textContent).toBe(
    entry.result.error!.message,
  );
  expect(screen.getByRole("table", { name: "Recovery by claim" })).toBeTruthy();
});
it("shows the loading and empty states", () => {
  view({ waterfall: { result: null, loading: true } });
  expect(screen.getByText("Allocating…")).toBeTruthy();
  expect(screen.getByText("No workout LGD")).toBeTruthy();
  expect(screen.getByText("No recovery statistics")).toBeTruthy();
});
it("keys each analysis on the session and the complete terms", () => {
  const client = { call: vi.fn(), close() {} } as unknown as FinstackClient;
  const options = allocateRecoveryOptions(client, 3, waterfallTerms);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "allocateRecovery",
    waterfallTerms,
  ]);
  expect(
    allocateRecoveryOptions(client, 4, waterfallTerms).queryKey,
  ).not.toEqual(options.queryKey);
  expect(allocateRecoveryOptions(client, 3, null).enabled).toBe(false);
  expect(
    workoutLgdOptions(client, 3, { ...workoutTerms, ead: 1 }).queryKey,
  ).not.toEqual(workoutLgdOptions(client, 3, workoutTerms).queryKey);
  expect(workoutLgdOptions(client, 3, null).enabled).toBe(false);
  expect(seniorityRecoveryStatsOptions(client, 3, null).enabled).toBe(false);
});
function Connected() {
  const [estate, setEstate] = useState(waterfallTerms);
  const [workout, setWorkout] = useState(workoutTerms);
  const [seniority, setSeniority] = useState(statsTerms);
  const waterfallQuery = useAllocateRecovery(estate);
  const workoutQuery = useWorkoutLgd(workout);
  const statsQuery = useSeniorityRecoveryStats(seniority);
  const ids = useSeniorityCalibrationIds();
  return (
    <Cards
      onWaterfallSubmit={setEstate}
      onWorkoutSubmit={setWorkout}
      onStatsSubmit={setSeniority}
      waterfall={{
        result: waterfallQuery.data,
        loading: waterfallQuery.isFetching,
        error: waterfallQuery.error?.message,
      }}
      workout={{
        result: workoutQuery.data,
        loading: workoutQuery.isFetching,
        error: workoutQuery.error?.message,
      }}
      stats={{
        result: statsQuery.data,
        loading: statsQuery.isFetching,
        error: statsQuery.error?.message,
      }}
      calibrationIds={ids.data}
    />
  );
}
it("runs the three analyses and a native rejection through the hooks", async () => {
  render(
    <FinstackQueryProvider>
      <Connected />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("table", { name: "Recovery by claim" });
  await screen.findByRole("table", { name: "Workout" });
  await screen.findByRole("table", { name: "Beta recovery" });
  fireEvent.change(screen.getByLabelText("Estate value"), {
    target: { value: "50" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Allocate" }));
  await waitFor(
    () =>
      expect(screen.getByRole("alert").textContent).toBe(
        (
          fixture.waterfall.collateralAboveEstate.result as NativeOutcome & {
            ok: false;
          }
        ).error.message,
      ),
    { timeout: budget(10000) },
  );
});
