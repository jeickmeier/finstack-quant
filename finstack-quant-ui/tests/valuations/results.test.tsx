// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
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
it("renders all supplied values, stamps, dates and qualified keys without inferred units", () => {
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
    expect(row.textContent).toContain(String(value));
    expect(row.textContent).not.toContain("Unit unavailable");
  }
  expect(
    screen.getByText("Raw measure values · units unavailable"),
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
  const ytm = screen.getByText("ytm").closest("tr")!;
  expect(ytm.textContent).toContain("decimal");
  const dv01 = screen.getByText("dv01").closest("tr")!;
  expect(dv01.textContent).toContain("currency");
  expect(screen.getByRole("columnheader", { name: "Pricing" })).toBeTruthy();
  expect(
    screen.getByRole("columnheader", { name: "Sensitivity" }),
  ).toBeTruthy();
  expect(
    screen.queryByText("Raw measure values · units unavailable"),
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
  const row = screen.getByText("ytm").closest("tr")!;
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
  expect(screen.getByRole("columnheader", { name: "Pricing" })).toBeTruthy();
  expect(screen.getByRole("table", { name: "Pricing measures" })).toBeTruthy();
  expect(
    screen.queryByRole("heading", { name: "Pricing measures" }),
  ).toBeNull();
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

function bucketCell(key: string, column = 1) {
  return screen.getByText(key, { exact: true }).closest("tr")!.children[
    column
  ] as HTMLElement;
}
function bucketFill(key: string, column = 1) {
  return bucketCell(key, column).querySelector<HTMLElement>(
    "[data-bucket-bar-fill]",
  );
}
it("draws signed qualified bucket rows, preserves opaque labels and leaves scalars/missing/nonfinite values ungraphed", () => {
  const measures = {
    "bucketed_dv01::USD_x3a_x3aOIS::5y": -100,
    "bucketed_dv01::USD_x3a_x3aOIS::2y": 50,
    "bucketed_dv01::USD_x3a_x3aOIS::zero": 0,
    "bucketed_cs01::ACME::4y@2028-01-05@quote-1": -2,
    "bucketed_dv01::USD_x3a_x3aOIS": 1000,
    bucketed_dv01: 1000,
    "cs01::ACME": -200,
    "bucketed_dv01::USD::5y::extra": 200,
    "bucketed_dv01::::5y": 200,
    "bucketed_cs01::ACME::": 200,
    "unrelated::ACME::5y": 200,
    "bucketed_dv01::USD::bad": NaN,
    "bucketed_cs01::ACME::infinite": Infinity,
  };
  const comparisonMeasures = { "bucketed_dv01::OTHER::1y": 2 };
  render(
    <MeasuresGrid
      result={{ ...fixture.result, measures }}
      compareTo={{
        ...fixture.result,
        measures: comparisonMeasures,
      }}
      metadata={metadataFor(measures)}
      comparisonMetadata={metadataFor(comparisonMeasures)}
    />,
  );
  for (const [key, value] of Object.entries(measures)) {
    expect(
      bucketCell(key)
        .querySelector(":scope > span[title]")
        ?.getAttribute("title"),
    ).toBe(String(value));
  }
  expect(bucketFill("bucketed_dv01::USD_x3a_x3aOIS::5y")?.style.cssText).toBe(
    "left: 0%; width: 50%;",
  );
  expect(bucketFill("bucketed_dv01::USD_x3a_x3aOIS::2y")?.style.cssText).toBe(
    "left: 50%; width: 25%;",
  );
  expect(
    bucketCell("bucketed_dv01::USD_x3a_x3aOIS::zero")
      .querySelector("[data-bucket-bar]")
      ?.getAttribute("aria-hidden"),
  ).toBe("true");
  expect(bucketFill("bucketed_dv01::USD_x3a_x3aOIS::zero")).toBeNull();
  expect(
    bucketFill("bucketed_cs01::ACME::4y@2028-01-05@quote-1")?.style.width,
  ).toBe("50%");
  for (const key of Object.keys(measures).slice(4))
    expect(bucketCell(key).querySelector("[data-bucket-bar]")).toBeNull();
  expect(bucketCell("bucketed_dv01::OTHER::1y").textContent).toBe(
    "—Unit unavailable",
  );
  expect(
    bucketCell("bucketed_dv01::OTHER::1y").querySelector("[data-bucket-bar]"),
  ).toBeNull();
});
it("scales each risk family, exact identifier, valuation and currency independently", () => {
  const measures = {
    "bucketed_dv01::A::1y": -100,
    "bucketed_dv01::A::2y": 50,
    "bucketed_dv01::A_x3aB::1y": -1,
    "bucketed_dv01::A_x5fx3aB::1y": 1000,
    "bucketed_cs01::A::1y": -2,
    "bucketed_cs01::A::2y": 1,
  };
  const comparisonMeasures = {
    "bucketed_dv01::A::1y": -1,
    "bucketed_dv01::A::2y": 2,
  };
  render(
    <MeasuresGrid
      result={{ ...fixture.result, measures }}
      compareTo={{
        ...fixture.result,
        value: { amount: "1", currency: "EUR" },
        measures: comparisonMeasures,
      }}
      metadata={metadataFor(measures)}
      comparisonMetadata={metadataFor(comparisonMeasures)}
    />,
  );
  for (const key of [
    "bucketed_dv01::A::1y",
    "bucketed_dv01::A_x3aB::1y",
    "bucketed_dv01::A_x5fx3aB::1y",
    "bucketed_cs01::A::1y",
  ])
    expect(bucketFill(key)?.style.width).toBe("50%");
  expect(bucketFill("bucketed_cs01::A::2y")?.style.width).toBe("25%");
  expect(bucketFill("bucketed_dv01::A::1y", 2)?.style.width).toBe("25%");
  expect(bucketFill("bucketed_dv01::A::2y", 2)?.style.width).toBe("50%");
  expect(
    bucketCell("bucketed_cs01::A::1y", 2).querySelector("[data-bucket-bar]"),
  ).toBeNull();
  expect(
    screen.getByRole("region", { name: "Comparison valuation" }).textContent,
  ).toContain("EUR 1");
});
it("separates supplied units and handles zero-only, huge and subnormal values without invalid widths", () => {
  const measures = {
    "bucketed_dv01::A::one": 100,
    "bucketed_dv01::A::two": 50,
    "bucketed_dv01::A::other-unit": 1,
    "bucketed_dv01::A::other-source": 2,
    "bucketed_dv01::A::unknown-unit": 0.01,
    "bucketed_cs01::huge::one": -Number.MAX_VALUE,
    "bucketed_cs01::huge::two": Number.MAX_VALUE / 2,
    "bucketed_cs01::tiny::one": Number.MIN_VALUE,
    "bucketed_cs01::zero::one": 0,
    "bucketed_cs01::zero::two": -0,
  };
  render(
    <MeasuresGrid
      result={{ ...fixture.result, measures }}
      metadata={metadataFor(measures)}
      units={{
        "bucketed_dv01::A::one": { label: "USD/bp", source: "native-one" },
        "bucketed_dv01::A::two": { label: "USD/bp", source: "native-one" },
        "bucketed_dv01::A::other-unit": {
          label: "EUR/bp",
          source: "native-one",
        },
        "bucketed_dv01::A::other-source": {
          label: "USD/bp",
          source: "native-other",
        },
      }}
    />,
  );
  expect(bucketFill("bucketed_dv01::A::two")?.style.width).toBe("25%");
  for (const key of [
    "bucketed_dv01::A::one",
    "bucketed_dv01::A::other-unit",
    "bucketed_dv01::A::other-source",
    "bucketed_dv01::A::unknown-unit",
    "bucketed_cs01::huge::one",
    "bucketed_cs01::tiny::one",
  ])
    expect(bucketFill(key)?.style.width).toBe("50%");
  expect(bucketFill("bucketed_cs01::huge::two")?.style.width).toBe("25%");
  for (const key of ["bucketed_cs01::zero::one", "bucketed_cs01::zero::two"]) {
    expect(bucketCell(key).querySelector("[data-bucket-bar]")).toBeTruthy();
    expect(bucketFill(key)).toBeNull();
  }
  expect(
    document.querySelector('[style*="NaN"], [style*="Infinity"]'),
  ).toBeNull();
});
