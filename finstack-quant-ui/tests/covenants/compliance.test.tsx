// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, expect, it } from "vitest";
import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { CovenantCompliance } from "@/components/finstack/covenants/components/covenant-compliance/covenant-compliance";
import { serializeHost } from "../../src/codec.mjs";
import type { DatedCovenantReportsWire } from "@/lib/finstack/generated/types/dated_covenant_reports";
import captured from "../../src/fixtures/covenants/analyst-covenant-series.json";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const engineJson = readFileSync(
  resolve(
    import.meta.dirname,
    "../../src/fixtures/covenants/analyst-covenants.json",
  ),
  "utf8",
);
// JSON imports widen string unions; the fixture is a native series.
const series = captured as unknown as DatedCovenantReportsWire[];
const cellText = (row: HTMLElement) =>
  [...row.querySelectorAll("th,td")].map((cell) => cell.textContent);
function evaluateSeries(rows: unknown[]): DatedCovenantReportsWire[] {
  const engine = native.CovenantEngine.fromJson(engineJson);
  try {
    return engine.evaluateSeries(rows);
  } finally {
    engine.free();
  }
}

afterEach(cleanup);

it("captures the native series the generator's row policy produces", () => {
  // First test date 2024Q3; leverage is non-finite there and is dropped, which
  // the engine allows because the leverage window opens on 2024-10-01.
  expect(series.map((row) => row.as_of)).toEqual([
    "2024-09-30",
    "2024-12-31",
    "2025-03-31",
    "2025-06-30",
    "2025-09-30",
    "2025-12-31",
  ]);
  expect(Object.keys(series[0].reports)).toEqual(["min_interest_coverage"]);
  const rows = series.map((row) => ({
    date: row.as_of,
    metrics: Object.fromEntries(
      Object.entries(row.reports).map(([key, report]) => [
        key === "max_debt_to_ebitda" ? "leverage" : "interest_coverage",
        report.actual_value,
      ]),
    ),
  }));
  expect(serializeHost(evaluateSeries(rows))).toBe(serializeHost(series));
  // The same leverage NaN inside the window is rejected at the boundary.
  expect(() =>
    evaluateSeries([
      {
        date: "2024-12-31",
        metrics: { leverage: Number.NaN, interest_coverage: 5 },
      },
    ]),
  ).toThrow(/NaN/);
});

it("grids returned verdicts by covenant and test date, with a dash where no report was returned", () => {
  const table = render(<CovenantCompliance series={series} />).getByRole(
    "table",
    { name: "Covenant compliance" },
  );
  const rows = within(table).getAllByRole("row");
  expect(cellText(rows[0])).toEqual([
    "Covenant",
    ...series.map((row) => row.as_of),
  ]);
  expect(cellText(rows[1])).toEqual([
    "min_interest_coverage",
    ...series.map(() => "Passed"),
  ]);
  expect(cellText(rows[2])).toEqual([
    "max_debt_to_ebitda",
    "—",
    ...series.slice(1).map(() => "Passed"),
  ]);
});

it("shows a native failure and the selected date's reports", () => {
  const failing = evaluateSeries([
    {
      date: "2025-03-31",
      metrics: { leverage: 4, interest_coverage: 5 },
    },
    {
      date: "2025-06-30",
      metrics: { leverage: 3, interest_coverage: 2 },
    },
  ]);
  const view = render(<CovenantCompliance series={failing} />);
  const grid = view.getByRole("table", { name: "Covenant compliance" });
  expect(within(grid).getAllByRole("row").slice(1).map(cellText)).toEqual([
    ["max_debt_to_ebitda", "Failed", "Passed"],
    ["min_interest_coverage", "Passed", "Failed"],
  ]);
  // The latest date is selected initially.
  expect(
    view
      .getByRole("button", { name: "2025-06-30" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  view.getByRole("table", { name: "Covenant reports on 2025-06-30" });
  fireEvent.click(view.getByRole("button", { name: "2025-03-31" }));
  const report = view.getByRole("table", {
    name: "Covenant reports on 2025-03-31",
  });
  expect(cellText(within(report).getAllByRole("row")[1])).toEqual([
    "max_debt_to_ebitda",
    failing[0].reports.max_debt_to_ebitda.covenant_type,
    "4",
    "3.5",
    String(failing[0].reports.max_debt_to_ebitda.headroom),
    "Failed",
    "—",
  ]);
});

it("states an empty series", () => {
  render(<CovenantCompliance series={[]} />);
  expect(document.body.textContent).toContain("No covenant reports returned");
  expect(document.querySelectorAll("table")).toHaveLength(1);
});
