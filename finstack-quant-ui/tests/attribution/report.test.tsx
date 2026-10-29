// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import type { PnlAttribution } from "finstack-quant-wasm";
import {
  PnlAttributionReport,
  attributionFactorFields,
  type PnlAttributionReportProps,
} from "@/components/finstack/attribution/components/pnl-attribution-report/pnl-attribution-report";
import fixture from "./cases.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
afterEach(cleanup);
const entry = (name: string) =>
  fixture.cases.find((candidate) => candidate.name === name)!;
const attribution = (name: string) =>
  entry(name).attribution as unknown as PnlAttribution;
// Attributions are regenerated and compared by `mise run ui-fixtures --check`
// (tests/attribution/generate.mjs).
it("pins the JSON inputs the attribution fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/attribution/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it("lists every factor field the Rust report knows", () => {
  // Each listed field is a returned Money bucket.
  for (const field of attributionFactorFields)
    expect(attribution("parallel")[field]).toHaveProperty("amount");
  expect(fixture.cases.map((c) => c.name)).toEqual([
    "parallel",
    "waterfall",
    "metrics_based",
    "taylor",
  ]);
});
it("shows the returned totals and factor buckets as exact money with Rust's shares", () => {
  const pnl = attribution("metrics_based");
  render(
    <PnlAttributionReport
      attribution={pnl}
      pctOfTotal={
        entry("metrics_based")
          .pctOfTotal as PnlAttributionReportProps["pctOfTotal"]
      }
      density="comfortable"
    />,
  );
  expect(screen.getByLabelText("Total P&L").title).toBe(pnl.total_pnl.amount);
  expect(screen.getByLabelText("Mark-to-market P&L").title).toBe(
    pnl.mark_to_market_pnl!.amount,
  );
  const table = screen.getByRole("table", { name: "P&L by factor" });
  const rows = (
    within(table).getAllByRole("row") as HTMLTableRowElement[]
  ).slice(1);
  expect(rows).toHaveLength(attributionFactorFields.length);
  const credit = rows[attributionFactorFields.indexOf("credit_curves_pnl")]!;
  expect(within(credit).getByTitle(pnl.credit_curves_pnl.amount)).toBeTruthy();
  expect(credit.cells[2]!.textContent).toBe(
    String(entry("metrics_based").pctOfTotal.credit_curves_pnl),
  );
  // The share is Rust's, for the factor's own returned amount.
  expect(entry("metrics_based").pctOfTotal.credit_curves_pnl).toBe(
    native.pnlAttributionPctOfTotal(pnl, Number(pnl.credit_curves_pnl.amount)),
  );
  expect(
    screen
      .getAllByRole("img")
      .map((figure) => figure.getAttribute("aria-label")),
  ).toEqual(["P&L by factor"]);
  // No waterfall without Rust steps.
  expect(screen.queryByRole("region", { name: "Waterfall" })).toBeNull();
});
it("draws the waterfall only from Rust's steps and lists them as returned", () => {
  const pnl = attribution("waterfall");
  render(<PnlAttributionReport attribution={pnl} />);
  const waterfall = screen.getByRole("region", { name: "Waterfall" });
  const steps = within(waterfall).getByRole("table", {
    name: "Waterfall steps",
  });
  const rows = (
    within(steps).getAllByRole("row") as HTMLTableRowElement[]
  ).slice(1);
  expect(rows).toHaveLength(pnl.waterfall_steps!.length);
  const title = (cell: HTMLTableCellElement) =>
    cell.querySelector("[title]")!.getAttribute("title");
  pnl.waterfall_steps!.forEach((step, index) => {
    const cells = rows[index]!.cells;
    expect(title(cells[2]!)).toBe(step.pv_before.amount);
    expect(title(cells[3]!)).toBe(step.pv_after.amount);
    expect(title(cells[4]!)).toBe(step.step_pnl.amount);
  });
  expect(
    screen
      .getAllByRole("img")
      .map((figure) => figure.getAttribute("aria-label")),
  ).toEqual(["P&L by factor", "Running present value by waterfall step"]);
  expect(rows[2]!.cells[1]!.textContent).toBe("Credit curves");
});
it("shows Rust's residual verdict only when supplied, else the returned figures", () => {
  const pnl = attribution("taylor");
  const { rerender } = render(<PnlAttributionReport attribution={pnl} />);
  const residual = () => screen.getByLabelText("Residual");
  expect(residual().textContent).not.toMatch(/tolerance:|within|outside/i);
  expect(residual().textContent).toContain(String(pnl.meta.residual_pct));
  expect(within(residual()).getByTitle(pnl.residual.amount)).toBeTruthy();
  rerender(
    <PnlAttributionReport
      attribution={pnl}
      residualWithinTolerance={entry("taylor").residualWithinTolerance}
      explain={entry("taylor").explain}
    />,
  );
  expect(entry("taylor").residualWithinTolerance).toBe(true);
  expect(residual().textContent).toContain("Residual within tolerance");
  rerender(
    <PnlAttributionReport attribution={pnl} residualWithinTolerance={false} />,
  );
  expect(residual().textContent).toContain("Residual outside tolerance");
  rerender(
    <PnlAttributionReport
      attribution={{ ...pnl, result_invalid: true }}
      residualWithinTolerance
    />,
  );
  expect(residual().textContent).toContain("Result invalid");
});
it("shows the explain text, notes, stamp and JSON as returned", () => {
  const pnl = attribution("taylor");
  render(
    <PnlAttributionReport
      attribution={pnl}
      explain={entry("taylor").explain}
    />,
  );
  expect(screen.getByRole("region", { name: "Explain" }).textContent).toContain(
    "Total P&L: USD -1060.42",
  );
  const notes = screen.getByRole("region", { name: "Notes" });
  expect(within(notes).getAllByRole("listitem")).toHaveLength(
    pnl.meta.notes.length,
  );
  expect(
    screen.getByText(JSON.stringify(pnl.meta.method), { selector: "span" }),
  ).toBeTruthy();
  const json = screen.getByRole("region", { name: "Attribution JSON" });
  expect(JSON.parse(json.querySelector("code")!.textContent!)).toEqual(pnl);
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<PnlAttributionReport attribution={null} />);
  expect(screen.getByRole("status").textContent).toBe("No P&L attribution");
  rerender(<PnlAttributionReport attribution={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Attributing P&L…");
  rerender(
    <PnlAttributionReport
      attribution={attribution("parallel")}
      error="Validation error: method"
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(
    "Validation error: method",
  );
});
