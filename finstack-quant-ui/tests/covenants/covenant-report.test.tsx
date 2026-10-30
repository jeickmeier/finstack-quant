// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it } from "vitest";
import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { CovenantReport } from "@/components/finstack/covenants/components/covenant-report/covenant-report";
import { serializeHost } from "../../src/codec.mjs";
import fixture from "../valuations/details/cases.json";
import type { DatedCovenantReportsWire } from "@/lib/finstack/generated/types/dated_covenant_reports";
import captured from "../../src/fixtures/covenants/analyst-covenant-series.json";

// JSON imports widen string unions; the fixture is a native series.
const series = captured as unknown as DatedCovenantReportsWire[];

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);

function showOriginal(scope: HTMLElement = document.body) {
  for (const button of within(scope).queryAllByRole("button", {
    name: "Original",
    hidden: true,
  }))
    fireEvent.click(button);
}
const cellText = (row: HTMLElement) =>
  [...row.querySelectorAll("th,td")].map((cell) => cell.textContent);

afterEach(cleanup);

it("preserves actual native covenant reports without computing verdicts", () => {
  const r = fixture.covenantRequest;
  const reports = native.evaluateEngine(r.engineJson, r.metricsJson, r.asOf);
  expect(serializeHost(reports)).toBe(fixture.covenantReportsJson);
  render(<CovenantReport reports={reports} />);
  showOriginal();
  expect(document.querySelector("pre")!.textContent).toBe(
    fixture.covenantReportsJson,
  );
});

it("tabulates each returned report with Rust's verdict and fields as supplied", () => {
  const reports = JSON.parse(fixture.covenantReportsJson);
  const table = render(<CovenantReport reports={reports} />).getByRole(
    "table",
    { name: "Covenant reports" },
  );
  const rows = within(table).getAllByRole("row");
  expect(cellText(rows[0])).toEqual([
    "Covenant",
    "Test",
    "Actual",
    "Threshold",
    "Headroom",
    "Native status",
    "Details",
  ]);
  // The returned breach: actual 6 against 5, headroom -0.2, details unset.
  expect(cellText(rows[1])).toEqual([
    "max_debt_ebitda",
    "Debt/EBITDA <= 5.00x",
    "6",
    "5",
    "-0.2",
    "Failed",
    "—",
  ]);
});

it("keeps engine order and shows the returned verdict even when it disagrees with the numbers", () => {
  const reports = series.at(-1)!.reports;
  expect(Object.keys(reports)).toEqual([
    "max_debt_to_ebitda",
    "min_interest_coverage",
  ]);
  const edited = {
    min_interest_coverage: {
      ...reports.min_interest_coverage,
      passed: false,
      headroom: null,
    },
    max_debt_to_ebitda: { ...reports.max_debt_to_ebitda, threshold: null },
  };
  const table = render(
    <CovenantReport
      reports={edited}
      caption="Edited"
      formatValue={(field, value) => `${field}:${value}`}
    />,
  ).getByRole("table", { name: "Edited" });
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows.map((row) => cellText(row)[0])).toEqual([
    "min_interest_coverage",
    "max_debt_to_ebitda",
  ]);
  expect(cellText(rows[0]).slice(2, 6)).toEqual([
    `actual_value:${reports.min_interest_coverage.actual_value}`,
    "threshold:3",
    "—",
    "Failed",
  ]);
  expect(cellText(rows[1]).slice(3, 6)).toEqual([
    "—",
    `headroom:${reports.max_debt_to_ebitda.headroom}`,
    "Passed",
  ]);
});

it("states an empty report set", () => {
  render(<CovenantReport reports={{}} />);
  expect(document.body.textContent).toContain("No covenant reports returned");
});
