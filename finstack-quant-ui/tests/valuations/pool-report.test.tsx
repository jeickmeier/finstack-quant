// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import type { PoolReportFieldUnits, valuations } from "finstack-quant-wasm";
import fixture from "./scenario-table/pool-report.json";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { PoolReport } from "@/components/finstack/valuations/components/pool-report/pool-report";

type Report = ReturnType<typeof valuations.instruments.poolReport>;
const byName = (name: string) => {
  const entry = fixture.cases.find((c) => c.name === name)!;
  return {
    report: entry.report as Report,
    units: entry.units as PoolReportFieldUnits,
  };
};
afterEach(cleanup);
const rows = (caption: string) =>
  within(screen.getByRole("table", { name: caption }))
    .getAllByRole("row")
    .slice(1)
    .map((row) =>
      [...row.querySelectorAll("th,td")].map((cell) => cell.textContent),
    );

it("shows the returned statistics with the units Rust states", () => {
  const { report, units } = byName("golden");
  render(<PoolReport report={report} units={units} />);
  const term = (name: string) =>
    screen.getByText(name, { selector: "dt" }).parentElement!;
  expect(term("Total balance")).toHaveProperty(
    "textContent",
    "Total balance20,000,000USD",
  );
  expect(within(term("WARF")).getByTitle(String(report.warf))).toBeTruthy();
  expect(term("WARF").textContent).toContain(units.warf.label);
  for (const unit of units.stats) {
    const tile = term(labelFor(unit.field));
    expect(
      within(tile).getByTitle(
        String(report.stats[unit.field as keyof typeof report.stats]),
      ),
    ).toBeTruthy();
    expect(tile.textContent).toContain(unit.label);
  }
});

it("keeps Rust's row order and names each column's unit from Rust", () => {
  const { report, units } = byName("golden");
  render(<PoolReport report={report} units={units} />);
  for (const [caption, table] of [
    ["By obligor", report.by_obligor],
    ["By industry", report.by_industry],
    ["By rating", report.by_rating],
  ] as const) {
    expect(rows(caption)).toEqual(
      table.map((row) => [
        row.key,
        Intl.NumberFormat("en-US").format(BigInt(row.balance.amount)),
        String(row.balance_pct),
        String(row.asset_count),
      ]),
    );
  }
  const headers = within(screen.getByRole("table", { name: "By rating" }))
    .getAllByRole("columnheader")
    .map((header) => header.textContent);
  expect(headers).toEqual([
    "Rating",
    ...units.concentration.map(
      (unit) => `${labelFor(unit.field)} (${unit.label})`,
    ),
  ]);
});

it("shows a null key as missing rather than dropping its balance", () => {
  const { report, units } = byName("unattributed");
  expect(report.by_obligor[0]!.key).toBeNull();
  render(<PoolReport report={report} units={units} />);
  expect(rows("By obligor")).toEqual([["Missing", "100,000,000", "100", "1"]]);
});

it("renders an empty pool with zero balances and empty tables", () => {
  const { report, units } = byName("empty");
  render(<PoolReport report={report} units={units} />);
  expect(screen.getAllByText("No assets in the pool")).toHaveLength(3);
  expect(
    screen.getByText("Total balance", { selector: "dt" }).parentElement!
      .textContent,
  ).toContain("0USD");
});

it("hides a stat Rust omits and shows a native error as returned", () => {
  const { report, units } = byName("golden");
  const { undrawn_commitment: _omitted, ...stats } = report.stats;
  const { rerender } = render(
    <PoolReport report={{ ...report, stats }} units={units} />,
  );
  expect(screen.queryByText("Undrawn commitment")).toBeNull();
  rerender(<PoolReport report={null} units={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Calculating…");
  rerender(<PoolReport report={null} units={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
