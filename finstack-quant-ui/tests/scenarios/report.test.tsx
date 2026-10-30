// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import type { ApplicationReport } from "finstack-quant-wasm";
import { MarketScenarioReport } from "@/components/finstack/scenarios/components/market-scenario-report/market-scenario-report";
import fixture from "./application/cases.json";
afterEach(cleanup);
const report = (name: string) =>
  fixture.cases.find((entry) => entry.name === name)!
    .report as unknown as ApplicationReport;
it("shows the returned counters, shocks, time roll and carry as returned", () => {
  const roll = report("roll-1m-25bp");
  render(<MarketScenarioReport report={roll} density="comfortable" />);
  expect(screen.getByText("User operations").nextSibling!.textContent).toBe(
    String(roll.user_operations),
  );
  expect(screen.getByText("Effects applied").nextSibling!.textContent).toBe(
    String(roll.operations_applied),
  );
  expect(
    screen.getByText("Valuation date changed").nextSibling!.textContent,
  ).toBe("yes");
  const shocks = screen.getByRole("table", { name: "Applied shocks" });
  const rows = within(shocks).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(roll.applied_shocks!.length);
  expect(rows[0]!.textContent).toContain("curve_id USD-OIS");
  expect(rows[0]!.textContent).toContain("value 25");
  expect(rows[0]!.textContent).toContain("unit bp");
  const timeRoll = screen.getByRole("region", { name: "Time roll" });
  expect(within(timeRoll).getByText("2025-01-01")).toBeTruthy();
  expect(within(timeRoll).getByText("2025-02-03")).toBeTruthy();
  expect(
    within(timeRoll).getByText("Calendar days").nextSibling!.textContent,
  ).toBe("33");
  // Returned exact amounts, never summed or rounded here.
  const carry = within(timeRoll).getByRole("table", {
    name: "Instrument carry",
  });
  expect(within(carry).getByText("US912828XG33")).toBeTruthy();
  expect(within(timeRoll).getAllByTitle("4613.745264131")).toHaveLength(2);
  expect(screen.getByText("No warnings")).toBeTruthy();
  const json = screen.getByRole("region", { name: "Application report" });
  expect(JSON.parse(json.querySelector("code")!.textContent!)).toEqual(roll);
});
it("lists every warning variant generically by kind and returned fields", () => {
  render(<MarketScenarioReport report={report("hz-par-cds-25bp")} />);
  const warnings = screen.getByRole("table", { name: "Warnings" });
  const row = within(warnings).getAllByRole("row")[1]!;
  expect(row.textContent).toContain("hazard_spread_first_order");
  expect(row.textContent).toContain("curve_id HZ");
  expect(row.textContent).toContain("recovery_rate 0.4");
  expect(screen.queryByRole("region", { name: "Time roll" })).toBeNull();
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<MarketScenarioReport report={null} />);
  expect(screen.getByRole("status").textContent).toBe("No scenario applied");
  rerender(<MarketScenarioReport report={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Applying scenario…");
  rerender(
    <MarketScenarioReport
      report={report("roll-1m-25bp")}
      error="Validation error: hazard curve 'HZ' has no lossless calibration recipe"
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain(
    "no lossless calibration recipe",
  );
});
