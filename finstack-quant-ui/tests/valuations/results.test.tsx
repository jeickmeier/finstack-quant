// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { createRequire } from "node:module";
import fixture from "../../src/fixtures/results/bond.json";
import {
  ValuationSummary,
  type SuppliedValuation,
} from "@/components/finstack/valuations/components/valuation-summary/valuation-summary";
import {
  MeasuresGrid,
  groupMeasures,
} from "@/components/finstack/valuations/components/measures-grid/measures-grid";
import type { MetricMetadata } from "finstack-quant-wasm";
import { returnedRounding } from "@/components/finstack/valuations/primitives/stamp-badge/stamp-badge";
import { formatMoney, formatRawMoney } from "../../src/format/format";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const metadataFor = (measures: Record<string, number>) =>
  Object.keys(measures).flatMap((key) => {
    try {
      return native.metricMetadata([key]);
    } catch {
      return [];
    }
  });
afterEach(cleanup);
it("matches the retained fixture against the real Node facade apart from wall-clock timestamp", () => {
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
it("keeps every supplied key and raw value when metadata is unavailable", () => {
  render(<MeasuresGrid result={fixture.result} />);
  expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
    formatRawMoney(fixture.result.value),
  );
  for (const text of [
    fixture.result.instrument_id,
    fixture.result.as_of,
    fixture.result.meta.numeric_mode,
    fixture.result.meta.rounding.mode,
    fixture.result.meta.version,
    fixture.result.meta.timestamp,
  ])
    expect(screen.getAllByText(text).length).toBeGreaterThan(0);
  const rows = screen
    .getAllByRole("row")
    .filter((row) => within(row).queryAllByRole("cell").length);
  expect(rows).toHaveLength(Object.keys(fixture.result.measures).length);
  for (const [key, value] of Object.entries(fixture.result.measures)) {
    const row = screen.getByText(key).closest("tr")!;
    expect(
      row.querySelector("td:nth-child(2) span[title]")?.getAttribute("title"),
    ).toBe(String(value));
    expect(row.textContent).not.toContain("Unit unavailable");
  }
  expect(
    screen.getByText(/Raw measure values · units unavailable/),
  ).toBeTruthy();
  expect(
    groupMeasures({ result: fixture.result })
      .flatMap((group) => group.rows)
      .map((row) => row.key)
      .sort(),
  ).toEqual(Object.keys(fixture.result.measures).sort());
});
it("renders Rust-supplied units and groups from native metric metadata", () => {
  render(
    <MeasuresGrid
      result={fixture.result}
      metadata={fixture.metadata as MetricMetadata[]}
    />,
  );
  const ytm = screen.getByText("Yield to maturity").closest("tr")!;
  expect(ytm.textContent).toContain("decimal");
  const dv01 = screen.getByText("DV01").closest("tr")!;
  expect(dv01.textContent).toContain("currency");
  expect(screen.getByText("Modified duration")).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Pricing" })).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Sensitivity" })).toBeTruthy();
  expect(
    screen.queryByText(/Raw measure values · units unavailable/),
  ).toBeNull();
  expect(screen.queryByText("Unit unavailable")).toBeNull();
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
      comparisonFormattedValue={formatMoney(
        other.value,
        returnedRounding(other.meta),
        native,
      )}
      metadata={fixture.metadata as MetricMetadata[]}
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
  const row = screen.getByText("Yield to maturity").closest("tr")!;
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
  expect(screen.getByTitle("1.23456789").textContent).toBe("USD 1.23456789");
  expect(screen.getAllByText("Unavailable").length).toBeGreaterThan(0);
  expect(
    view.container.querySelector('[data-density="comfortable"]'),
  ).toBeTruthy();
  view.rerender(<ValuationSummary result={{ ...result, meta: null }} />);
  expect(screen.getByText("Metadata unavailable")).toBeTruthy();
  view.rerender(
    <MeasuresGrid result={null} loading error="Supplied failure" />,
  );
  expect(screen.getByText("No valuation supplied")).toBeTruthy();
  expect(screen.getByText("No measures supplied")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toBe("Supplied failure");
});
it("uses only explicit per-result units and keeps ungrouped metrics in the unavailable group", () => {
  const result = {
    ...fixture.result,
    measures: { ytm: 0.043, "custom::key": -1 },
  };
  render(
    <MeasuresGrid
      result={result}
      compareTo={result}
      metadata={metadataFor(result.measures)}
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
  const row = screen.getByText("Yield to maturity").closest("tr")!;
  expect(row.textContent).toContain("0.043decimal rate");
  expect(row.textContent).toContain("0.043Unit unavailable");
});

it("retains independent supplied model context and exact large money while presenting grouped column headers", () => {
  const result = {
    ...fixture.result,
    value: { amount: "9007199254740993.1234567890123456789", currency: "USD" },
    meta: null,
    measures: { ytm: 0.043 },
  };
  const view = render(
    <MeasuresGrid
      result={result}
      compareTo={{ ...result, instrument_id: "COMPARE" }}
      model="discounting"
      comparisonModel="independent-model"
      metadata={metadataFor(result.measures)}
    />,
  );
  const primary = screen.getByRole("region", { name: "Valuation" });
  const comparison = screen.getByRole("region", {
    name: "Comparison valuation",
  });
  expect(within(primary).getByTitle(result.value.amount).textContent).toBe(
    "USD 9,007,199,254,740,993.1234567890123456789",
  );
  expect(within(primary).getByText("discounting")).toBeTruthy();
  expect(within(primary).queryByText("independent-model")).toBeNull();
  expect(within(comparison).getByText("independent-model")).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Pricing" })).toBeTruthy();
  expect(screen.getByRole("table", { name: "Pricing measures" })).toBeTruthy();
  view.rerender(
    <MeasuresGrid
      result={result}
      compareTo={result}
      model="discounting"
      metadata={metadataFor(result.measures)}
    />,
  );
  expect(
    within(
      screen.getByRole("region", { name: "Comparison valuation" }),
    ).queryByText("discounting"),
  ).toBeNull();
});

it("shortens measure values to six significant digits while retaining exact returned values", () => {
  const measures = { ytm: 0.123456789, dv01: -103.45856181590352 };
  render(
    <MeasuresGrid
      result={{ ...fixture.result, measures }}
      metadata={metadataFor(measures)}
    />,
  );
  const ytm = screen.getByText("Yield to maturity").closest("tr")!;
  const value = within(ytm).getByTitle("0.123456789");
  expect(value.querySelector(".print\\:hidden")?.textContent).toBe("0.123457");
  expect(value.getAttribute("aria-label")).toContain("exact value 0.123456789");
  expect(
    within(screen.getByText("DV01").closest("tr")!)
      .getByTitle("-103.45856181590352")
      .querySelector(".print\\:hidden")?.textContent,
  ).toBe("-103.459");

  const exact = screen.getByRole("button", { name: "Exact values" });
  expect(exact.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(exact);
  expect(exact.getAttribute("aria-pressed")).toBe("true");
  expect(within(ytm).getByTitle("0.123456789").textContent).toContain(
    "0.123456789",
  );
});

it("sorts metadata-backed bucket tenors, collapses long groups and reveals exact keys", () => {
  render(
    <MeasuresGrid
      result={fixture.result}
      metadata={fixture.metadata as MetricMetadata[]}
    />,
  );
  const bucket = screen.getByRole("button", {
    name: /Bucketed DV01 · USD-OIS/,
  });
  const bucketPanel = document.getElementById(
    bucket.getAttribute("aria-controls")!,
  );
  expect(bucket.getAttribute("aria-expanded")).toBe("false");
  expect(bucketPanel?.classList.contains("hidden")).toBe(true);
  fireEvent.click(bucket);
  expect(bucket.getAttribute("aria-expanded")).toBe("true");
  expect(bucketPanel?.classList.contains("hidden")).toBe(false);
  const table = screen.getByRole("table", {
    name: "Sensitivity: Bucketed DV01 · USD-OIS",
  });
  expect(
    within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => row.querySelector("td:first-child span")?.textContent),
  ).toEqual([
    "3m",
    "6m",
    "1y",
    "2y",
    "3y",
    "5y",
    "7y",
    "10y",
    "15y",
    "20y",
    "30y",
  ]);
  expect(within(table).getByText("3m").getAttribute("title")).toBe(
    "bucketed_dv01::USD-OIS::3m",
  );

  const exactKeys = screen.getByRole("button", { name: "Exact keys" });
  expect(exactKeys.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(exactKeys);
  expect(exactKeys.getAttribute("aria-pressed")).toBe("true");
  expect(within(table).getByText("bucketed_dv01::USD-OIS::3m")).toBeTruthy();
  expect(screen.getByText("ytm")).toBeTruthy();
  expect(screen.queryByText("Yield to maturity")).toBeNull();
  expect(document.querySelector("[data-bucket-bar]")).toBeNull();

  fireEvent.click(bucket);
  expect(bucket.getAttribute("aria-expanded")).toBe("false");
  expect(bucketPanel?.classList.contains("hidden")).toBe(true);
});

it("filters only supplied zero pairs and preserves rows with a missing comparison value", () => {
  const primaryMeasures = { ytm: 0, dv01: 0 };
  const comparisonMeasures = { ytm: 0, other: 0 };
  render(
    <MeasuresGrid
      result={{ ...fixture.result, measures: primaryMeasures }}
      compareTo={{
        ...fixture.result,
        instrument_id: "OTHER",
        measures: comparisonMeasures,
      }}
      metadata={metadataFor(primaryMeasures)}
      comparisonMetadata={metadataFor(comparisonMeasures)}
    />,
  );
  const showAll = screen.getByRole("button", { name: "Show all" });
  const nonzero = screen.getByRole("button", { name: "Nonzero only" });
  expect(showAll.getAttribute("aria-pressed")).toBe("true");
  expect(nonzero.getAttribute("aria-pressed")).toBe("false");
  expect(screen.getByText("Yield to maturity")).toBeTruthy();

  fireEvent.click(nonzero);
  expect(nonzero.getAttribute("aria-pressed")).toBe("true");
  expect(showAll.getAttribute("aria-pressed")).toBe("false");
  expect(screen.queryByText("Yield to maturity")).toBeNull();
  const dv01 = screen.getByText("DV01").closest("tr")!;
  expect(within(dv01).getAllByRole("cell")[2].textContent).toBe("—");
  const other = screen.getByText("other").closest("tr")!;
  expect(within(other).getAllByRole("cell")[1].textContent).toBe("—");
  expect(other.textContent).toContain("0");

  fireEvent.click(showAll);
  expect(showAll.getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByText("Yield to maturity")).toBeTruthy();
});
