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
import fixture from "./scenario-table/deal-charts.json";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { CoverageTestChart } from "@/components/finstack/valuations/components/coverage-test-chart/coverage-test-chart";

vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  () => ({
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
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
afterEach(cleanup);
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

it("says a trigger in first_breaches has no ratio by date rather than drawing it", () => {
  const run = runOf("covered");
  const recorded = run.diagnostics.periods!.flatMap((p) => p.coverage_tests);
  const silent = run.diagnostics.first_breaches.find(
    (b) => !recorded.some((row) => row.test_id === b.test_id),
  )!;
  render(<CoverageTestChart run={run} units={units} />);
  choose(silent.test_id);
  expect(
    screen.getByText(new RegExp(`no ratio, trigger or cushion for`)),
  ).toBeTruthy();
  expect(
    screen.getByText(new RegExp(`First breach ${silent.first_breach}`)),
  ).toBeTruthy();
  expect(
    screen.queryByRole("img", { name: new RegExp(silent.test_id) }),
  ).toBeNull();
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
    expect(screen.getByRole("status").textContent).toMatch(
      /^The simulation returned no coverage tests/,
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
