// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { createRequire } from "node:module";
import fixture from "../src/fixtures/results/bond.json";
import {
  ValuationSummary,
  type SuppliedValuation,
} from "../registry/components/valuation-summary/valuation-summary";
import {
  MeasuresGrid,
  groupMeasures,
} from "../registry/components/measures-grid/measures-grid";
import { returnedRounding } from "../registry/primitives/stamp-badge/stamp-badge";
import { formatMoney } from "../src/format/format";
afterEach(cleanup);
it("matches the retained fixture against the real Node facade apart from wall-clock timestamp", () => {
  const native = createRequire(import.meta.url)(
    "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const request = fixture.request;
  const current = native.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    request.asOf,
    request.model,
    request.metrics,
  );
  const { timestamp: _, ...meta } = current.meta;
  const { timestamp: _stored, ...stored } = fixture.result.meta;
  expect({ ...current, meta }).toEqual({ ...fixture.result, meta: stored });
  expect(native.listStandardMetricsGrouped()).toEqual(fixture.groups);
});
it("renders all supplied values, stamps, dates and qualified keys without inferred units", () => {
  render(<MeasuresGrid result={fixture.result} groups={fixture.groups} />);
  expect(
    screen.getByText(
      formatMoney(fixture.result.value, returnedRounding(fixture.result.meta)),
    ),
  ).toBeTruthy();
  for (const text of [
    fixture.result.instrument_id,
    fixture.result.as_of,
    fixture.result.meta.numeric_mode,
    fixture.result.meta.rounding.mode,
    fixture.result.meta.version,
    fixture.result.meta.timestamp,
  ])
    expect(screen.getByText(text)).toBeTruthy();
  const rows = screen
    .getAllByRole("row")
    .filter((row) => within(row).queryAllByRole("cell").length);
  expect(rows).toHaveLength(Object.keys(fixture.result.measures).length);
  for (const [key, value] of Object.entries(fixture.result.measures)) {
    const row = screen.getByText(key).closest("tr")!;
    expect(row.textContent).toContain(String(value));
    expect(row.textContent).toContain("Unit unavailable");
  }
  expect(
    groupMeasures({ result: fixture.result, groups: fixture.groups })
      .flatMap((group) => group.rows)
      .map((row) => row.key)
      .sort(),
  ).toEqual(Object.keys(fixture.result.measures).sort());
});
it("keeps comparison currencies, dates and stamps independent and never calculates missing values", () => {
  const other: SuppliedValuation = {
    instrument_id: "EUR-OTHER",
    as_of: "2026-03-31",
    value: { amount: "1.9999", currency: "EUR" },
    measures: { ytm: 0.043, other: 12 },
    meta: {
      numeric_mode: "supplied-mode",
      rounding: { mode: "ceil", output_scale_by_currency: { EUR: 3 } },
      fx_policy_applied: "supplied-fx-policy",
      version: "supplied-version",
    },
  };
  render(
    <MeasuresGrid
      result={fixture.result}
      compareTo={other}
      groups={fixture.groups}
    />,
  );
  const primary = screen.getByRole("region", {
    name: "Valuation",
  });
  const comparison = screen.getByRole("region", {
    name: "Comparison valuation",
  });
  expect(primary.textContent).toContain("USD 1,042,500");
  for (const value of [
    "EUR 2.000",
    other.instrument_id,
    other.as_of,
    "supplied-mode",
    "ceil",
    "supplied-fx-policy",
    "supplied-version",
  ])
    expect(comparison.textContent).toContain(value);
  const row = screen.getByText("ytm").closest("tr")!;
  expect(row.textContent).toContain("0.043");
  expect(row.textContent).not.toContain("4.3%");
  const missing = screen.getByText("other").closest("tr")!;
  expect(within(missing).getAllByRole("cell")[1].textContent).toContain("—");
  expect(screen.queryByText(/^(Difference|Ratio|Aggregate|Total)$/)).toBeNull();
});
it("handles missing and partial metadata without invented rounding or stamps", () => {
  const result = {
    ...fixture.result,
    value: { amount: "1.23456789", currency: "USD" },
    meta: { numeric_mode: "f64", rounding: { mode: "bankers" } },
  };
  const view = render(
    <ValuationSummary result={result} compact density="comfortable" />,
  );
  expect(screen.getByText("USD 1.23456789")).toBeTruthy();
  expect(screen.getAllByText("Unavailable").length).toBeGreaterThan(0);
  expect(
    view.container.querySelector('[data-density="comfortable"]'),
  ).toBeTruthy();
  view.rerender(<ValuationSummary result={{ ...result, meta: null }} />);
  expect(screen.getByText("Metadata unavailable")).toBeTruthy();
  view.rerender(
    <MeasuresGrid
      result={null}
      groups={fixture.groups}
      loading
      error="Supplied failure"
    />,
  );
  expect(screen.getByText("No valuation supplied")).toBeTruthy();
  expect(screen.getByText("No measures supplied")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toBe("Supplied failure");
});
it("uses only explicit per-result units and puts unknown or ambiguous metric groups in the unavailable group", () => {
  const result = {
    ...fixture.result,
    measures: { ytm: 0.043, "custom::key": -1 },
  };
  render(
    <MeasuresGrid
      result={result}
      compareTo={result}
      groups={{ A: ["ytm"], B: ["ytm"] }}
      units={{
        ytm: {
          label: "decimal rate",
          source: "supplied native metric contract",
        },
      }}
    />,
  );
  expect(
    screen.getByRole("table", { name: "Group unavailable measures" }),
  ).toBeTruthy();
  const row = screen.getByText("ytm").closest("tr")!;
  expect(row.textContent).toContain("0.043decimal rate");
  expect(row.textContent).toContain("0.043Unit unavailable");
});
