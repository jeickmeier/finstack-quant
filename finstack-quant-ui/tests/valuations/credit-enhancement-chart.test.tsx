// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import type {
  SimulationFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import { createChartRuntime } from "@tanstack/charts";
import fixture from "./scenario-table/deal-charts.json";
import { formatRaw } from "@/lib/finstack/format/format";
import { CreditEnhancementChart } from "@/components/finstack/valuations/components/credit-enhancement-chart/credit-enhancement-chart";

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

type Run = ReturnType<StructuredCredit["runSimulation"]>;
const runOf = (name: string) =>
  fixture.cases.find((c) => c.name === name)!.run as unknown as Run;
const units = fixture.units as SimulationFieldUnits;
afterEach(() => {
  cleanup();
  definitions.clear();
});
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

it("plots each tranche's returned enhancement and the period's excess spread on their dates", () => {
  const run = runOf("covered");
  const periods = run.diagnostics.periods!;
  render(<CreditEnhancementChart run={run} units={units} />);
  withScene("Credit enhancement by tranche and date", (scene, definition) => {
    const points = scene.points.filter(
      (point: any) => point.markId === "credit-enhancement",
    );
    const rows = periods.flatMap((period) =>
      period.credit_enhancement.map((row) => ({
        ...row,
        payment_date: period.payment_date,
      })),
    );
    const seen = (point: any) => [
      point.datum.tranche_id,
      new Date(point.xValue).toISOString().slice(0, 10),
      point.yValue,
    ];
    const sorted = (list: unknown[][]) =>
      list.map((row) => JSON.stringify(row)).sort();
    expect(sorted(points.map(seen))).toEqual(
      sorted(
        rows.map((row) => [
          row.tranche_id,
          row.payment_date,
          row.credit_enhancement,
        ]),
      ),
    );
    // One colour per tranche Rust returned.
    expect(new Set(points.map((point: any) => point.color)).size).toBe(
      periods[0]!.credit_enhancement.length,
    );
    expect(
      definition.tooltip.content([{ datum: rows[0] }, { datum: 0 }]).rows,
    ).toEqual([
      {
        label: `${rows[0]!.tranche_id} · ${rows[0]!.payment_date}`,
        value: formatRaw(rows[0]!.credit_enhancement),
      },
    ]);
  });
  withScene("Excess spread by date", (scene, definition) => {
    expect(
      scene.points
        .filter((point: any) => point.markId === "excess-spread")
        .map((point: any) => [
          new Date(point.xValue).toISOString().slice(0, 10),
          point.yValue,
        ]),
    ).toEqual(periods.map((p) => [p.payment_date, p.excess_spread]));
    expect(
      definition.tooltip.content([{ datum: periods[0] }, { datum: 0 }]).rows,
    ).toEqual([
      {
        label: periods[0]!.payment_date,
        value: formatRaw(periods[0]!.excess_spread),
      },
    ]);
  });
});

it("lists a real payment period's one row without calling it a settlement", () => {
  const run = runOf("one-period");
  const [period] = run.diagnostics.periods!;
  expect(run.diagnostics.periods).toHaveLength(1);
  expect(period!.excess_spread).not.toBe(0);
  render(<CreditEnhancementChart run={run} units={units} />);
  const status = screen.getByRole("status").textContent!;
  expect(status).toBe(
    `The simulation returned one period, on ${period!.payment_date}. A chart by date needs two, so what it returned is listed instead.`,
  );
  expect(rows().slice(1)).toEqual(expected(run));
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
