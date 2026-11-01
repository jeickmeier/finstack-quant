// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import type {
  SimulationFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import { createChartRuntime } from "@tanstack/charts";
import fixture from "./scenario-table/deal-charts.json";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { CoverageTestChart } from "@/components/finstack/valuations/components/coverage-test-chart/coverage-test-chart";

// Keeps each chart's definition, which the real chart runtime renders below.
const definitions = vi.hoisted(() => new Map<string, unknown>());
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  () => ({
    FinstackChart: ({
      ariaLabel,
      definition,
    }: {
      ariaLabel: string;
      definition: unknown;
    }) => {
      definitions.set(ariaLabel, definition);
      return <div role="img" aria-label={ariaLabel} />;
    },
  }),
);
vi.mock(
  "@/components/finstack/shared/primitives/enum-field/enum-field",
  () => ({
    EnumField: ({
      label,
      value,
      onValueChange,
      options,
    }: {
      label: string;
      value: string;
      onValueChange: (value: string) => void;
      options: { value: string; label: string }[];
    }) => (
      <label>
        {label}
        <select
          value={value}
          onChange={(event) => onValueChange(event.target.value)}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </label>
    ),
  }),
);

type Run = ReturnType<StructuredCredit["runSimulation"]>;
const runOf = (name: string) =>
  fixture.cases.find((c) => c.name === name)!.run as unknown as Run;
const units = fixture.units as SimulationFieldUnits;
afterEach(() => {
  cleanup();
  definitions.clear();
});
/** Render a captured definition with the real runtime and hand its scene to `read`. */
function withScene<T>(
  label: string,
  read: (scene: any, definition: any) => T,
): T {
  const definition = definitions.get(label) as any;
  const runtime = createChartRuntime<any, any, any>();
  try {
    return read(
      runtime.render(definition, { width: 800, height: 300 }),
      definition,
    );
  } finally {
    runtime.destroy();
  }
}
const markGroup = (scene: any, key: string) =>
  scene.nodes
    .find((node: any) => node.key === "marks")
    .children.find((node: any) => node.key === key);
const testRows = (run: Run, id: string) =>
  run.diagnostics.periods!.flatMap((period) =>
    period.coverage_tests
      .filter((row) => row.test_id === id)
      .map((row) => ({ ...row, payment_date: period.payment_date })),
  );
const utc = (date: string) => new Date(`${date}T00:00:00Z`).toISOString();
const cells = (row: Element) =>
  [...row.querySelectorAll("th,td")].map((cell) => cell.textContent);
const bodyRows = (caption: string) =>
  within(screen.getByRole("table", { name: caption }))
    .getAllByRole("row")
    .slice(1)
    .map(cells);
const choose = (id: string) =>
  fireEvent.change(screen.getByLabelText("Test"), { target: { value: id } });
const diverted = (run: Run) =>
  run.diagnostics.periods!.flatMap((period) =>
    period.waterfall.diverted_amounts.map((row) => [
      period.payment_date,
      row.source_tier,
      row.target_tranche,
      groupDecimal(row.amount.amount),
      row.reason,
    ]),
  );

it("captures a real breach, a real diversion and a trigger with no ratio", () => {
  const { first_breaches, periods } = runOf("covered").diagnostics;
  expect(first_breaches.some((b) => b.first_breach !== null)).toBe(true);
  expect(diverted(runOf("covered")).length).toBeGreaterThan(0);
  const recorded = periods!.flatMap((p) => p.coverage_tests);
  expect(
    first_breaches.some(
      (b) =>
        b.first_breach !== null &&
        !recorded.some((row) => row.test_id === b.test_id),
    ),
  ).toBe(true);
  // OC_SENIOR passes, breaches and cures, and its date stays.
  const senior = recorded.filter((row) => row.test_id === "OC_SENIOR");
  const failing = senior.findIndex((row) => !row.passing);
  expect(failing).toBeGreaterThan(0);
  expect(senior.slice(failing).some((row) => row.passing)).toBe(true);
  expect(
    first_breaches.find((b) => b.test_id === "OC_SENIOR")!.first_breach,
  ).toBe(
    periods!.find((p) =>
      p.coverage_tests.some((t) => t.test_id === "OC_SENIOR" && !t.passing),
    )!.payment_date,
  );
});

it("lists every test with Rust's first breach, in Rust's order", () => {
  const run = runOf("covered");
  render(<CoverageTestChart run={run} units={units} />);
  expect(bodyRows("First breach by test")).toEqual(
    run.diagnostics.first_breaches.map((b) => [
      b.test_id,
      b.first_breach ?? "Never failed",
    ]),
  );
});

it("shows the selected test's returned rows, flag and unit as Rust states them", () => {
  const run = runOf("covered");
  const later = run.diagnostics.first_breaches.find(
    (b) => b.test_id === "OC_SENIOR",
  )!;
  const recorded = run.diagnostics.periods!.flatMap((period) =>
    period.coverage_tests
      .filter((row) => row.test_id === later.test_id)
      .map((row) => [
        period.payment_date,
        formatRaw(row.ratio),
        formatRaw(row.trigger_level),
        formatRaw(row.cushion),
        row.passing ? "Yes" : "No",
      ]),
  );
  render(<CoverageTestChart run={run} units={units} />);
  choose(later.test_id);
  expect(recorded.length).toBeGreaterThan(0);
  expect(bodyRows(`${later.test_id} by payment date`)).toEqual(recorded);
  expect(
    screen.getByRole("img", {
      name: `${later.test_id} ratio against trigger by date`,
    }),
  ).toBeTruthy();
  expect(
    screen.getByRole("img", { name: `${later.test_id} cushion by date` }),
  ).toBeTruthy();
  expect(
    screen.getByText(
      `First breach ${later.first_breach}. A later cure does not clear it.`,
    ),
  ).toBeTruthy();
  for (const { label } of units.coverage_tests)
    expect(
      within(
        screen.getByRole("table", { name: `${later.test_id} by payment date` }),
      ).getAllByText(new RegExp(label)).length,
    ).toBeGreaterThan(0);
});

it("plots the returned ratio, cushion and trigger on their dates, coloured by Rust's flag", () => {
  const run = runOf("covered");
  const rows = testRows(run, "OC_SENIOR");
  expect(rows.some((row) => row.passing)).toBe(true);
  expect(rows.some((row) => !row.passing)).toBe(true);
  render(<CoverageTestChart run={run} units={units} initialTest="OC_SENIOR" />);
  const points = (scene: any, mark: string) =>
    scene.points.filter((point: any) => point.markId === mark);
  const dated = (point: any) => [
    new Date(point.xValue).toISOString(),
    point.yValue,
  ];
  withScene("OC_SENIOR ratio against trigger by date", (scene, definition) => {
    expect(points(scene, "ratio").map(dated)).toEqual(
      rows.map((row) => [utc(row.payment_date), row.ratio]),
    );
    expect(points(scene, "trigger-level").map(dated)).toEqual(
      rows.map((row) => [utc(row.payment_date), row.trigger_level]),
    );
    // One colour per Rust flag, in the fixed Passed/Failed order.
    expect(scene.colors.domain).toEqual(["Passed", "Failed"]);
    const colour = new Map(
      points(scene, "ratio").map((point: any, i: number) => [
        rows[i]!.passing,
        point.color,
      ]),
    );
    expect(colour.size).toBe(2);
    points(scene, "ratio").forEach((point: any, i: number) =>
      expect(point.color).toBe(colour.get(rows[i]!.passing)),
    );
    // The first breach is a rule on Rust's date, at the x of that date's dot.
    const breach = run.diagnostics.first_breaches.find(
      (b) => b.test_id === "OC_SENIOR",
    )!.first_breach!;
    const atBreach = points(scene, "ratio")[
      rows.findIndex((row) => row.payment_date === breach)
    ];
    const [rule] = markGroup(scene, "first-breach").children;
    expect(rule.x1).toBeCloseTo(atBreach.x, 6);
    expect(rule.x2).toBeCloseTo(atBreach.x, 6);
    // The tooltip reads the row as Rust returned it.
    const row = rows[1]!;
    expect(
      definition.tooltip.content([{ datum: row }, { datum: 3 }]).rows,
    ).toEqual([
      { label: "Payment date", value: row.payment_date },
      { label: "Ratio", value: formatRaw(row.ratio) },
      { label: "Trigger level", value: formatRaw(row.trigger_level) },
      { label: "Cushion", value: formatRaw(row.cushion) },
    ]);
  });
  withScene("OC_SENIOR cushion by date", (scene) => {
    expect(points(scene, "cushion").map(dated)).toEqual(
      rows.map((row) => [utc(row.payment_date), row.cushion]),
    );
    expect(markGroup(scene, "zero-cushion").children).toHaveLength(1);
    expect(markGroup(scene, "first-breach")).toBeTruthy();
  });
  // A test that never failed has no first-breach rule.
  choose("IC_SENIOR");
  withScene("IC_SENIOR ratio against trigger by date", (scene) =>
    expect(markGroup(scene, "first-breach")).toBeUndefined(),
  );
});

it("plots Rust's diverted cash by date, zero included", () => {
  const run = runOf("covered");
  render(<CoverageTestChart run={run} units={units} />);
  withScene("Diverted cash by date", (scene, definition) => {
    const periods = run.diagnostics.periods!;
    expect(
      scene.points
        .filter((point: any) => point.markId === "diverted-cash")
        .map((point: any) => point.yValue),
    ).toEqual(periods.map((p) => Number(p.waterfall.diverted_cash.amount)));
    expect(
      definition.tooltip.content([{ datum: periods[0] }, { datum: 0 }]).rows,
    ).toEqual([
      {
        label: periods[0]!.payment_date,
        value: `${groupDecimal(periods[0]!.waterfall.diverted_cash.amount)} · coverage tier failed: ${periods[0]!.waterfall.had_diversions ? "Yes" : "No"}`,
      },
    ]);
  });
});

it("states no claim outstanding for a retired note, and draws no dot for it", () => {
  const run = runOf("retired-senior");
  const rows = testRows(run, "OC_SENIOR");
  const open = rows.filter((row) => row.ratio !== null);
  const closed = rows.filter((row) => row.ratio === null);
  expect(open.length).toBeGreaterThan(0);
  expect(closed.length).toBeGreaterThan(0);
  // Rust's flag is unchanged: the test passes with nothing to cover.
  expect(closed.every((row) => row.passing && row.cushion === null)).toBe(true);
  render(<CoverageTestChart run={run} units={units} initialTest="OC_SENIOR" />);
  expect(bodyRows("OC_SENIOR by payment date")).toEqual(
    rows.map((row) => [
      row.payment_date,
      row.ratio === null ? "No claim outstanding" : formatRaw(row.ratio),
      formatRaw(row.trigger_level),
      row.cushion === null ? "No claim outstanding" : formatRaw(row.cushion),
      "Yes",
    ]),
  );
  expect(
    screen.getByText(
      `No claim outstanding on ${closed.length} of the ${rows.length} recorded dates, so no dot is drawn on them.`,
    ),
  ).toBeTruthy();
  withScene("OC_SENIOR ratio against trigger by date", (scene, definition) => {
    const of = (mark: string) =>
      scene.points.filter((point: any) => point.markId === mark);
    expect(of("ratio").map((point: any) => point.yValue)).toEqual(
      open.map((row) => row.ratio),
    );
    // The trigger still stands on every recorded date.
    expect(of("trigger-level")).toHaveLength(rows.length);
    expect(definition.tooltip.content([{ datum: closed[0] }]).rows).toEqual([
      { label: "Payment date", value: closed[0]!.payment_date },
      { label: "Ratio", value: "No claim outstanding" },
      { label: "Trigger level", value: formatRaw(closed[0]!.trigger_level) },
      { label: "Cushion", value: "No claim outstanding" },
    ]);
  });
  withScene("OC_SENIOR cushion by date", (scene) =>
    expect(
      scene.points.filter((point: any) => point.markId === "cushion"),
    ).toHaveLength(open.length),
  );
});

it("says a trigger in first_breaches has no ratio by date rather than drawing it", () => {
  const run = runOf("covered");
  const recorded = run.diagnostics.periods!.flatMap((p) => p.coverage_tests);
  const silent = run.diagnostics.first_breaches.find(
    (b) => !recorded.some((row) => row.test_id === b.test_id),
  )!;
  render(<CoverageTestChart run={run} units={units} />);
  choose(silent.test_id);
  expect(
    screen.getByText(
      `Recorded on 0 of ${run.diagnostics.periods!.length} dates.`,
    ),
  ).toBeTruthy();
  expect(
    screen.getByText(new RegExp(`First breach ${silent.first_breach}`)),
  ).toBeTruthy();
  expect(
    screen.queryByRole("img", { name: new RegExp(silent.test_id) }),
  ).toBeNull();
});

it("gives the counts returned when a test covers only some of the deal's dates", () => {
  const run = runOf("covered");
  const periods = run.diagnostics.periods!;
  const partial = {
    ...run,
    diagnostics: {
      ...run.diagnostics,
      periods: periods.map((period, index) =>
        index === 0
          ? {
              ...period,
              coverage_tests: period.coverage_tests.filter(
                (test) => test.test_id !== "OC_SENIOR",
              ),
            }
          : period,
      ),
    },
  };
  render(
    <CoverageTestChart run={partial} units={units} initialTest="OC_SENIOR" />,
  );
  expect(
    screen.getByText(
      `Recorded on ${periods.length - 1} of ${periods.length} dates.`,
    ),
  ).toBeTruthy();
  cleanup();
  // A test on every date needs no note.
  render(<CoverageTestChart run={run} units={units} initialTest="OC_SENIOR" />);
  expect(screen.queryByText(/^Recorded on /)).toBeNull();
});

it("shows the deal's diversions with Rust's reason, for any selected test", () => {
  const run = runOf("covered");
  render(<CoverageTestChart run={run} units={units} />);
  expect(
    screen.getByRole("img", { name: "Diverted cash by date" }),
  ).toBeTruthy();
  expect(bodyRows("Diversions")).toEqual(diverted(run));
});

it("states a deal or settlement with no coverage tests instead of drawing", () => {
  for (const name of ["untested", "settlement"]) {
    render(<CoverageTestChart run={runOf(name)} units={units} />);
    expect(screen.getByRole("status").textContent).toBe(
      "No coverage test was evaluated.",
    );
    expect(screen.queryByRole("img")).toBeNull();
    cleanup();
  }
});

it("lists rather than charts a deal's one period", () => {
  const run = runOf("one-period");
  expect(run.diagnostics.periods).toHaveLength(1);
  render(<CoverageTestChart run={run} units={units} />);
  expect(
    screen.getByText(/The simulation returned one period, on/),
  ).toBeTruthy();
  expect(screen.queryByRole("img")).toBeNull();
  expect(bodyRows("First breach by test")).toEqual(
    run.diagnostics.first_breaches.map((b) => [
      b.test_id,
      b.first_breach ?? "Never failed",
    ]),
  );
});

it("keeps the states of no run and a native error", () => {
  render(<CoverageTestChart run={null} units={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Calculating…");
  cleanup();
  render(<CoverageTestChart run={null} units={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
