// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import type {
  SimulationFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import fixture from "./scenario-table/deal-charts.json";
import { formatRaw } from "@/lib/finstack/format/format";
import { CreditEnhancementChart } from "@/components/finstack/valuations/components/credit-enhancement-chart/credit-enhancement-chart";

vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  () => ({
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);

type Run = ReturnType<StructuredCredit["runSimulation"]>;
const runOf = (name: string) =>
  fixture.cases.find((c) => c.name === name)!.run as unknown as Run;
const units = fixture.units as SimulationFieldUnits;
afterEach(cleanup);
const table = () => screen.getByRole("table", { name: "By payment date" });
const rows = () =>
  within(table())
    .getAllByRole("row")
    .map((row) =>
      [...row.querySelectorAll("th,td")].map((cell) => cell.textContent),
    );
const expected = (run: Run) =>
  run.diagnostics.periods!.map((period) => [
    period.payment_date,
    formatRaw(period.excess_spread),
    ...period.credit_enhancement.map((row) =>
      formatRaw(row.credit_enhancement),
    ),
  ]);

it("captures several tranches and a moving excess spread", () => {
  const periods = runOf("covered").diagnostics.periods!;
  expect(periods[0]!.credit_enhancement.map((c) => c.tranche_id)).toEqual([
    "SENIOR",
    "MEZZ",
    "EQUITY",
  ]);
  expect(periods[0]!.excess_spread).not.toBe(periods[1]!.excess_spread);
});

it("charts and lists every returned value in Rust's order and units", () => {
  const run = runOf("covered");
  render(<CreditEnhancementChart run={run} units={units} />);
  expect(
    screen.getByRole("img", { name: "Credit enhancement by tranche and date" }),
  ).toBeTruthy();
  expect(
    screen.getByRole("img", { name: "Excess spread by date" }),
  ).toBeTruthy();
  const [header, ...body] = rows();
  expect(body).toEqual(expected(run));
  expect(header).toEqual([
    "Payment date",
    `Excess spread (${units.excess_spread.label})`,
    ...run.diagnostics.periods![0]!.credit_enhancement.map(
      (row) => `${row.tranche_id} (${units.credit_enhancement.label})`,
    ),
  ]);
});

it("lists rather than charts a settlement-only deal's one period", () => {
  const run = runOf("settlement");
  expect(run.diagnostics.periods).toHaveLength(1);
  render(<CreditEnhancementChart run={run} units={units} />);
  expect(screen.getByRole("status").textContent).toMatch(
    /^The simulation returned one period, on 2024-01-01/,
  );
  expect(screen.queryByRole("img")).toBeNull();
  expect(rows().slice(1)).toEqual(expected(run));
});

it("keeps the states of no run, no units and a native error", () => {
  render(<CreditEnhancementChart run={null} units={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Calculating…");
  cleanup();
  render(<CreditEnhancementChart run={runOf("covered")} units={null} />);
  expect(screen.getByRole("status").textContent).toBe("No units returned");
  cleanup();
  render(
    <CreditEnhancementChart run={null} units={null} error="native failure" />,
  );
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
