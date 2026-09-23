// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import type { MetricMetadata } from "finstack-quant-wasm";
import {
  DimensionalMeasures,
  dimensionalMeasureCount,
} from "@/components/finstack/valuations/components/dimensional-measures/dimensional-measures";
import fixture from "../../src/fixtures/results/bond.json";

afterEach(cleanup);

function descriptor(
  key: string,
  metric: string,
  components: string[],
  unit: MetricMetadata["unit"] = "currency",
): MetricMetadata {
  return {
    key,
    metric,
    components,
    unit,
    group: "Sensitivity",
    bucketed: false,
  };
}

it("shows every native bond bucket and its exact value in a compact tenor ladder", () => {
  const metadata = fixture.metadata as MetricMetadata[];
  expect(dimensionalMeasureCount({ result: fixture.result, metadata })).toBe(
    11,
  );
  render(<DimensionalMeasures result={fixture.result} metadata={metadata} />);

  expect(screen.getByRole("heading", { name: "Bucketed DV01" })).toBeTruthy();
  expect(screen.getByText("Exact Bucketed DV01 keys and values")).toBeTruthy();
  expect(
    screen.queryByRole("heading", { name: "Yield To Maturity" }),
  ).toBeNull();
  const ladder = screen.getByRole("table", {
    name: "Returned bucket values for USD-OIS",
  });
  const buckets = within(ladder)
    .getAllByRole("rowheader")
    .map((cell) => cell.textContent);
  expect(buckets).toEqual([
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
  expect(screen.getAllByText("currency").length).toBeGreaterThan(0);
  const exact = fixture.result.measures["bucketed_dv01::USD-OIS::30y"];
  expect(ladder.querySelector(`span[title="${exact}"]`)).toBeTruthy();
  expect(
    screen.getByRole("table", {
      name: "Complete returned metric keys and exact values",
    }).textContent,
  ).toContain("bucketed_dv01::USD-OIS::30y");
});

it("renders a native two-axis matrix without mistaking metric names for metadata", () => {
  const measures = {
    "bucketed_vega::volA::4500": 2,
    "bucketed_vega::volA::4600": -3,
    "bucketed_vega::volB::4500": 0,
    "bucketed_vega::volB::4600": 4,
  };
  const metadata = Object.keys(measures).map((key) => {
    const [, expiry, strike] = key.split("::");
    return descriptor(key, "bucketed_vega", [expiry!, strike!]);
  });
  render(<DimensionalMeasures result={{ measures }} metadata={metadata} />);

  const matrix = screen.getByRole("table", {
    name: "Returned two-coordinate measure matrix",
  });
  expect(screen.getByRole("heading", { name: "Bucketed Vega" })).toBeTruthy();
  expect(
    within(matrix)
      .getAllByRole("columnheader")
      .map((cell) => cell.textContent),
  ).toEqual(["Coordinate 1 / Coordinate 2", "4500", "4600"]);
  const volA = within(matrix)
    .getByRole("rowheader", { name: "volA" })
    .closest("tr")!;
  expect(volA.textContent).toContain("+2");
  expect(volA.textContent).toContain("-3");
  expect(matrix.querySelector('span[title="0"]')).toBeTruthy();
});

it("uses a flat coordinate table for sparse shapes and leaves missing comparisons blank", () => {
  const measures = {
    "risk::A::1y": 1,
    "risk::A::2y": 2,
    "risk::B::1y": 3,
  };
  const metadata = [
    descriptor("risk::A::1y", "risk", ["A", "1y"]),
    descriptor("risk::A::2y", "risk", ["A", "2y"]),
    descriptor("risk::B::1y", "risk", ["B", "1y"]),
  ];
  render(
    <DimensionalMeasures
      result={{ measures }}
      metadata={metadata}
      compareTo={{ measures: { "risk::A::1y": 9 } }}
      comparisonMetadata={metadata}
    />,
  );
  const table = screen.getByRole("table", {
    name: "Returned coordinates and measure values",
  });
  expect(within(table).getAllByRole("row")).toHaveLength(4);
  const missingComparison = within(table)
    .getByRole("row", { name: /A 2y/ })
    .querySelector("td:last-child")!;
  expect(missingComparison.textContent).toBe("—");
  expect(
    screen.queryByRole("table", {
      name: "Returned two-coordinate measure matrix",
    }),
  ).toBeNull();
});

it("selects a native three-axis slice and retains one-axis qualified measures", () => {
  const measures = {
    "surface::front::A::1y": 5,
    "surface::front::A::2y": 6,
    "surface::back::A::1y": 7,
    "surface::back::A::2y": 8,
    "constituent_delta::AAPL": 12,
  };
  const metadata = [
    ...Object.keys(measures)
      .filter((key) => key.startsWith("surface::"))
      .map((key) => descriptor(key, "surface", key.split("::").slice(1))),
    descriptor("constituent_delta::AAPL", "constituent_delta", ["AAPL"]),
  ];
  render(<DimensionalMeasures result={{ measures }} metadata={metadata} />);
  expect(dimensionalMeasureCount({ result: { measures }, metadata })).toBe(5);
  expect(
    screen.getByRole("heading", { name: "Constituent Delta" }),
  ).toBeTruthy();
  const slice = screen.getByRole("combobox", {
    name: "First returned coordinate",
  });
  expect((slice as HTMLSelectElement).value).toBe("back");
  const ladder = screen.getByRole("table", {
    name: "Returned bucket values for A",
  });
  expect(ladder.textContent).toContain("+7");
  expect(ladder.textContent).not.toContain("+5");
  fireEvent.change(slice, { target: { value: "front" } });
  expect(ladder.textContent).toContain("+5");
  expect(ladder.textContent).not.toContain("+7");
});

it("does not invent coordinates or units when metadata is unavailable", () => {
  render(<DimensionalMeasures result={{ measures: { "custom::A::B": 2 } }} />);
  expect(screen.getByText(/Metadata unavailable/)).toBeTruthy();
  expect(screen.getByText("custom::A::B")).toBeTruthy();
  expect(screen.queryByText("Coordinate 1")).toBeNull();
});
