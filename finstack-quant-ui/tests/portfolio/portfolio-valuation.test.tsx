// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import {
  cleanup,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MetricMetadata } from "finstack-quant-wasm";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  useValuePortfolio,
  valuePortfolioOptions,
  type PortfolioValuationReport,
  type PortfolioValuationRequest,
} from "@/hooks/portfolio/use-value-portfolio/use-value-portfolio";
import {
  portfolioMetricsSeriesOptions,
  usePortfolioMetricsSeries,
} from "@/hooks/portfolio/use-portfolio-metrics-series/use-portfolio-metrics-series";
import { usePortfolioSpecValidator } from "@/hooks/portfolio/use-portfolio-spec-validator/use-portfolio-spec-validator";
import { PortfolioSummary } from "@/components/finstack/portfolio/components/portfolio-summary/portfolio-summary";
import { PositionsTable } from "@/components/finstack/portfolio/components/positions-table/positions-table";
import {
  PortfolioMetrics,
  type PortfolioMetricSeriesData,
} from "@/components/finstack/portfolio/components/portfolio-metrics/portfolio-metrics";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import gallery from "./gallery.json";
import hostParity from "./host-parity.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const read = (path: string) =>
  JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
const pricingMarket = JSON.stringify(
  read("../valuations/instruments/pricing-market.json"),
);
const { inputs } = read(
  "../../../finstack-quant/portfolio/tests/fixtures/host_parity.json",
);
const parityMarket = JSON.stringify(inputs.market_t0);
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
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
type Resources = Record<
  "portfolioConstructed" | "portfolioFreed" | "constructed" | "freed",
  number
>;
let harness: Awaited<ReturnType<typeof startWorker>>;
const proxy = () =>
  harness.proxy as unknown as WorkerApi & { resources(): Promise<Resources> };
const resources = (): Promise<Resources> => proxy().resources();
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) =>
      unwrap(await (proxy() as any)[method](request)),
    ),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const galleryRequest: PortfolioValuationRequest = {
  specJson: JSON.stringify(gallery.spec),
  marketJson: pricingMarket,
  metrics: gallery.request.metrics,
};
const parityRequest: PortfolioValuationRequest = {
  specJson: JSON.stringify(inputs.portfolio_spec),
  marketJson: parityMarket,
  metrics: hostParity.request.metrics,
};
const galleryReport = gallery.report as unknown as PortfolioValuationReport;
const metadata = gallery.metadata as unknown as MetricMetadata[];
const gallerySeries = gallery.series as unknown as PortfolioMetricSeriesData;
const rowsOf = (table: HTMLElement) =>
  within(table).getAllByRole("row") as HTMLTableRowElement[];
// Fixtures are regenerated and compared by `mise run ui-fixtures --check`
// (tests/portfolio/generate.mjs).
it(
  "values the gallery book through the real worker as the generator did",
  async () => {
    const report = unwrap(await proxy().valuePortfolio(galleryRequest));
    expect(serializeHost(report)).toBe(serializeHost(gallery.report));
    const series = unwrap(
      await proxy().portfolioMetricsSeries({
        metrics: report.metrics,
        base: gallerySeries.base,
      }),
    );
    expect(series).toEqual(gallery.series.entries);
    // Rust's lexical bucket order, not tenor order.
    expect(series.slice(0, 3).map((entry) => entry.components[1])).toEqual([
      "10y",
      "15y",
      "1y",
    ]);
    // A metric on the menu the swap cannot compute is listed, not thrown.
    expect(
      report.valuation.position_values["IRS-1"].inapplicable_metrics,
    ).toEqual(["cs01"]);
  },
  budget(15000),
);
it(
  "matches the values the Py/WASM host-parity tests pin and reuses the cached book",
  async () => {
    const before = await resources();
    const report = unwrap(await proxy().valuePortfolio(parityRequest));
    expect(serializeHost(report)).toBe(serializeHost(hostParity.report));
    const { expected } = hostParity;
    expect(
      Number(report.valuation.position_values["POS-1"].value_base.amount),
    ).toBe(expected.valuation_position_value_base);
    expect(Number(report.valuation.by_entity.FUND.amount)).toBe(
      expected.valuation_entity_value,
    );
    expect(report.metrics.aggregated.dv01.total).toBe(
      expected.metrics_total_dv01,
    );
    expect(report.metrics.by_position["POS-1"].metrics.dv01).toBe(
      expected.metrics_position_dv01,
    );
    const first = await resources();
    expect(first.portfolioConstructed - before.portfolioConstructed).toBe(1);
    expect(first.portfolioFreed - before.portfolioFreed).toBe(0);
    // The canonical spec is a direct cache hit: no new Portfolio is built.
    unwrap(
      await proxy().valuePortfolio({
        ...parityRequest,
        specJson: native.parsePortfolioSpecJson(parityRequest.specJson),
      }),
    );
    const second = await resources();
    expect(second.portfolioConstructed).toBe(first.portfolioConstructed);
    expect(second.portfolioFreed).toBe(first.portfolioFreed);
  },
  budget(15000),
);
it("validates and canonicalizes specs natively, including the book invariants", async () => {
  const canonical = unwrap(
    await proxy().validatePortfolio(JSON.stringify(inputs.portfolio_spec)),
  );
  expect(canonical).toBe(
    native.parsePortfolioSpecJson(JSON.stringify(inputs.portfolio_spec)),
  );
  const malformed = await proxy().validatePortfolio("{");
  expect(malformed.ok).toBe(false);
  if (!malformed.ok) expect(malformed.error.kind).toBe("validation");
  const orphan = structuredClone(inputs.portfolio_spec);
  orphan.positions[0].entity_id = "NOPE";
  const unknown = await proxy().validatePortfolio(JSON.stringify(orphan));
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) {
    expect(unknown.error.kind).toBe("not_found");
    expect(unknown.error.message).toContain("unknown entity 'NOPE'");
  }
});
it("returns native errors for an unknown metric, a bad market and a bad series base", async () => {
  const metric = await proxy().valuePortfolio({
    ...parityRequest,
    metrics: ["not_a_metric"],
  });
  expect(metric.ok).toBe(false);
  const market = await proxy().valuePortfolio({
    ...parityRequest,
    marketJson: "{",
  });
  expect(market.ok).toBe(false);
  const series = await proxy().portfolioMetricsSeries({
    metrics: hostParity.report.metrics as never,
    base: "bucketed_dv01::",
  });
  expect(series.ok).toBe(false);
  if (!series.ok) expect(series.error.kind).toBe("validation");
});
it("keys both queries on the worker session and the complete snapshotted request", () => {
  const options = valuePortfolioOptions(null, 3, galleryRequest);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "valuePortfolio",
    galleryRequest,
  ]);
  expect(options.queryKey[3]).not.toBe(galleryRequest);
  expect(valuePortfolioOptions(null, 3, null).enabled).toBe(false);
  const seriesRequest = {
    metrics: galleryReport.metrics,
    base: "bucketed_dv01",
  };
  const series = portfolioMetricsSeriesOptions(null, 3, seriesRequest);
  // Metrics are plain numbers, so the key serializes.
  expect(() => JSON.stringify(series.queryKey)).not.toThrow();
  expect(series.queryKey[2]).toBe("portfolioMetricsSeries");
});
it("summarizes the total, the stamped FX policy and entities in registration order", () => {
  const reversed = { ...galleryReport, entityIds: ["RATES", "CREDIT"] };
  render(<PortfolioSummary report={reversed} />);
  // Exact returned money (MoneyValue keeps the raw amount as its title).
  expect(screen.getAllByTitle("1110934.085252535").length).toBeGreaterThan(0);
  expect(screen.getByText("cashflow_date")).toBeTruthy();
  const rows = rowsOf(
    within(screen.getByRole("region", { name: "Value by entity" })).getByRole(
      "table",
    ),
  );
  expect(rows.slice(1).map((row) => row.cells[0]!.textContent)).toEqual([
    "RATES",
    "CREDIT",
  ]);
  expect(within(rows[1]!).getByTitle("8912.21632401552")).toBeTruthy();
  expect(screen.getByText("No position fell back to PV only.")).toBeTruthy();
});
it("lists degraded positions with their native risk error", () => {
  const valuation = structuredClone(galleryReport.valuation);
  valuation.degraded_positions = ["IRS-1"];
  valuation.position_values["IRS-1"]!.risk_error = "theta failed";
  render(
    <PortfolioSummary report={{ entityIds: ["CREDIT", "RATES"], valuation }} />,
  );
  const degraded = screen.getByRole("region", { name: "Degraded positions" });
  expect(degraded.textContent).toContain("IRS-1 · theta failed");
});
it("orders positions by the supplied spec ids, not by position_values keys", async () => {
  // Integer-like ids enumerate numerically in JavaScript ("2" before "10").
  const valuation = structuredClone(galleryReport.valuation);
  valuation.position_values = {
    "10": { ...valuation.position_values["BOND-1"]!, position_id: "10" },
    "2": { ...valuation.position_values["IRS-1"]!, position_id: "2" },
  };
  expect(Object.keys(valuation.position_values)).toEqual(["2", "10"]);
  render(
    <PositionsTable
      report={{ positionIds: ["10", "2"], valuation }}
      metadata={metadata}
    />,
  );
  const rows = rowsOf(
    within(screen.getByRole("region", { name: "Positions" })).getAllByRole(
      "table",
    )[0]!,
  );
  expect(rows.slice(1).map((row) => row.cells[0]!.textContent)).toEqual([
    "10",
    "2",
  ]);
  expect(rows[2]!.textContent).toContain("cs01");
  // The first position's valuation_result is shown; activating a row selects it.
  const detail = screen.getByRole("region", { name: "Position 10" });
  expect(within(detail).getByText("US912828XG33")).toBeTruthy();
  await userEvent.click(rows[2]!);
  const swap = screen.getByRole("region", { name: "Position 2" });
  expect(within(swap).getByText("IRS-5Y-USD-STD")).toBeTruthy();
});
it("shows a position without a valuation result and the empty, loading and error states", () => {
  const valuation = structuredClone(galleryReport.valuation);
  valuation.position_values["BOND-1"]!.valuation_result = null;
  const { rerender } = render(
    <PositionsTable report={{ positionIds: ["BOND-1", "IRS-1"], valuation }} />,
  );
  expect(
    screen.getByText("No valuation result returned for this position"),
  ).toBeTruthy();
  rerender(<PositionsTable report={null} />);
  expect(screen.getByRole("status").textContent).toBe("No portfolio valuation");
  rerender(<PortfolioMetrics report={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Aggregating metrics…");
  rerender(<PortfolioSummary report={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
it("charts the series per curve in Rust order and lists aggregated, position and note values as returned", async () => {
  const metrics = structuredClone(galleryReport.metrics);
  metrics.skipped_metrics = [
    { position_id: "IRS-1", metric_id: "theta", value: Number.NaN },
  ];
  render(
    <PortfolioMetrics
      report={{ ...galleryReport, metrics }}
      series={gallerySeries}
      metadata={metadata}
    />,
  );
  expect(
    screen
      .getAllByRole("img")
      .map((figure) => figure.getAttribute("aria-label")),
  ).toEqual([
    "bucketed_dv01 USD-TREASURY portfolio totals by bucket",
    "bucketed_dv01 USD-OIS portfolio totals by bucket",
    "bucketed_dv01 USD-SOFR-3M portfolio totals by bucket",
  ]);
  const notes = screen.getByRole("region", { name: "Aggregation notes" });
  expect(notes.textContent).toContain("theta_period_days");
  expect(notes.textContent).toContain("IRS-1 · theta · NaN");
  const aggregated = within(
    screen.getByRole("region", { name: "Aggregated metrics" }),
  ).getByRole("table");
  const dv01 = rowsOf(aggregated).find(
    (row) => row.cells[0]!.textContent === "dv01",
  )!;
  expect([...dv01.cells].map((cell) => cell.textContent)).toEqual([
    "dv01",
    String(metrics.aggregated.dv01!.total),
    String(metrics.aggregated.dv01!.by_entity.CREDIT),
    String(metrics.aggregated.dv01!.by_entity.RATES),
  ]);
  const byPosition = screen.getByRole("region", {
    name: "Metrics by position",
  });
  expect(within(byPosition).getByText("cs01::US912828XG33")).toBeTruthy();
  await userEvent.click(
    within(byPosition).getByRole("combobox", { name: "Position" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "IRS-1" }));
  expect(within(byPosition).queryByText("cs01::US912828XG33")).toBeNull();
  expect(
    within(byPosition).getByText("bucketed_dv01::USD-SOFR-3M::5y"),
  ).toBeTruthy();
});
function Connected({ request }: { request: PortfolioValuationRequest }) {
  const report = useValuePortfolio(request);
  const series = usePortfolioMetricsSeries(
    report.data
      ? { metrics: report.data.metrics, base: "bucketed_dv01" }
      : null,
  );
  return (
    <>
      <PortfolioSummary
        report={report.data ?? null}
        loading={report.isFetching}
        error={report.error?.message}
      />
      <PortfolioMetrics
        report={report.data ?? null}
        series={
          series.data ? { base: "bucketed_dv01", entries: series.data } : null
        }
        error={series.error?.message}
      />
    </>
  );
}
it("renders the worker valuation and series through the hooks, then the native error", async () => {
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={parityRequest} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        screen.getByRole("img", {
          name: "bucketed_dv01 USD-OIS portfolio totals by bucket",
        }),
      ).toBeTruthy(),
    { timeout: budget(5000) },
  );
  expect(screen.getAllByTitle("3000227.82081091").length).toBeGreaterThan(0);
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected request={{ ...parityRequest, metrics: ["not_a_metric"] }} />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getAllByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
it("validates spec text through the shared native validator hook", async () => {
  const { result } = renderHook(() => usePortfolioSpecValidator(), {
    wrapper: FinstackQueryProvider,
  });
  await waitFor(
    async () =>
      expect(await result.current(JSON.stringify(inputs.portfolio_spec))).toBe(
        native.parsePortfolioSpecJson(JSON.stringify(inputs.portfolio_spec)),
      ),
    { timeout: budget(5000) },
  );
  await expect(result.current("{")).rejects.toThrow();
});
