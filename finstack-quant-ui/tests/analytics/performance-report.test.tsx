// @vitest-environment jsdom
import { createRequire } from "node:module";
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
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient } from "@tanstack/react-query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  performanceReportOptions,
  usePerformanceReport,
  type PerformanceReport as Report,
  type PerformanceReportRequest,
} from "@/hooks/analytics/use-performance-report/use-performance-report";
import {
  PerformanceReport,
  type PerformanceReportData,
} from "@/components/finstack/analytics/components/performance-report/performance-report";
import { unwrap } from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import defaults from "./defaults.json";
import explicitCase from "./explicit.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder (the heatmap preset still validates its domain).
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
  | "performanceConstructed"
  | "performanceFreed"
  | "fiscalConstructed"
  | "fiscalFreed",
  number
>;
let harness: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) => {
      if (method === "initialize")
        return unwrap(await harness.proxy.initialize());
      if (method === "performanceReport")
        return unwrap(
          await harness.proxy.performanceReport(
            request as PerformanceReportRequest,
          ),
        );
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const cases = { defaults, explicit: explicitCase };
const entry = (name: keyof typeof cases) => ({
  request: cases[name].request as PerformanceReportRequest,
  report: cases[name].report as unknown as Report,
});
const panel = entry("defaults").request;
// Reports are regenerated and compared by `mise run ui-fixtures --check`
// (tests/analytics/generate.mjs).
it.each(["defaults", "explicit"] as const)(
  "returns the %s native report through the real worker and frees every handle",
  async (name) => {
    const { request, report } = entry(name);
    const before = await resources();
    expect(unwrap(await harness.proxy.performanceReport(request))).toEqual(
      report,
    );
    const after = await resources();
    expect(after.performanceConstructed - before.performanceConstructed).toBe(
      1,
    );
    expect(after.performanceFreed - before.performanceFreed).toBe(1);
    expect(after.fiscalConstructed - before.fiscalConstructed).toBe(
      request.fiscalYearStart ? 1 : 0,
    );
    expect(after.fiscalFreed - before.fiscalFreed).toBe(
      after.fiscalConstructed - before.fiscalConstructed,
    );
  },
  budget(15000),
);
it("leaves omitted options to Rust and honours an explicit fiscal year", () => {
  const defaults = entry("defaults").report;
  const explicit = entry("explicit").report;
  // Rust's default rolling window (63) and episode count (up to five).
  const [fund] = defaults.tickers;
  expect(fund!.rollingSharpe.values).toHaveLength(
    fund!.activeDates.length - 62,
  );
  expect(fund!.drawdownEpisodes.length).toBeLessThanOrEqual(5);
  expect(explicit.tickers[0]!.drawdownEpisodes).toHaveLength(3);
  // The lookback ends on the last active date unless one is requested.
  expect(defaults.lookback.refDate).toBe(defaults.activeDates.at(-1));
  expect(explicit.lookback.refDate).toBe("2025-06-30");
  // Calendar-year FYTD by default; the April fiscal year changes it.
  expect(defaults.lookback.returns.fytd).toEqual(defaults.lookback.returns.ytd);
  expect(explicit.lookback.returns.fytd[0]).not.toBe(
    explicit.lookback.returns.ytd[0],
  );
  expect(defaults.benchmarkIdx).toBe(1);
});
it("builds a price panel with the native constructor and reports no benchmark when none is named", async () => {
  const dates = panel.dates.slice(0, 260);
  const prices = [dates.map((_, i) => 100 + 10 * Math.sin(i / 20) + i / 20)];
  const request: PerformanceReportRequest = {
    dates,
    values: prices,
    kind: "prices",
    tickerNames: ["Prices"],
  };
  const report = unwrap(await harness.proxy.performanceReport(request));
  const direct = new native.Performance(dates, prices, ["Prices"]);
  try {
    expect(report.summary).toEqual(direct.summary());
    expect(report.tickers[0]!.cumulativeReturns).toEqual(
      Array.from(direct.cumulativeReturns()[0]),
    );
    expect(report.activeDates).toEqual(direct.getActiveDates());
  } finally {
    direct.free();
  }
  expect(report.benchmarkIdx).toBeNull();
});
it("returns the native error and still frees the panel when a later call fails", async () => {
  const { request } = entry("explicit");
  const before = await resources();
  const failed = await harness.proxy.performanceReport({
    ...request,
    window: 0,
  });
  expect(failed.ok).toBe(false);
  if (!failed.ok)
    expect(failed.error.message).toContain("rolling window must be at least 1");
  const after = await resources();
  expect(after.performanceConstructed - before.performanceConstructed).toBe(1);
  expect(after.performanceFreed - before.performanceFreed).toBe(1);
  expect(after.fiscalFreed - before.fiscalFreed).toBe(
    after.fiscalConstructed - before.fiscalConstructed,
  );
  const unknown = await harness.proxy.performanceReport({
    ...request,
    benchmarkTicker: "Missing",
  });
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) expect(unknown.error.kind).toBe("not_found");
});
it("keys the query on the worker session and the complete snapshotted request", () => {
  const { request } = entry("explicit");
  const options = performanceReportOptions(null, 2, request);
  expect(options.queryKey).toEqual([
    "finstack",
    2,
    "performanceReport",
    JSON.stringify(request),
  ]);
  expect(performanceReportOptions(null, 2, null).enabled).toBe(false);
});
it("isolates non-finite inputs and sessions while reusing identical requests", async () => {
  const { request, report } = entry("explicit");
  const call = vi.fn(async () => report);
  const client = { call } as unknown as FinstackClient;
  const queries = new QueryClient();
  try {
    for (const riskFreeRate of [null, Number.NaN, Infinity, -Infinity]) {
      const input = { ...request, riskFreeRate };
      const options = performanceReportOptions(client, 2, input);
      await queries.query(options);
      await queries.query(
        performanceReportOptions(client, 2, structuredClone(input)),
      );
    }
    expect(call).toHaveBeenCalledTimes(4);
    await queries.query(
      performanceReportOptions(client, 3, { ...request, riskFreeRate: null }),
    );
    expect(call).toHaveBeenCalledTimes(5);
  } finally {
    queries.clear();
  }
});
it("sends the original numeric snapshot after the caller mutates its request", async () => {
  const { request, report } = entry("explicit");
  const input = {
    ...request,
    riskFreeRate: Number.NaN,
    values: request.values.map((row) => [...row]),
  };
  input.values[0]![0] = Infinity;
  const expected = structuredClone(input);
  const call = vi.fn(async () => report);
  const client = { call } as unknown as FinstackClient;
  const options = performanceReportOptions(client, 2, input);
  const key = options.queryKey[3];
  input.riskFreeRate = 0;
  input.values[0]![0] = 0;
  const queries = new QueryClient();
  try {
    await queries.query(options);
    expect(call).toHaveBeenCalledWith("performanceReport", expected);
    expect(options.queryKey[3]).toBe(key);
    expect(options.queryKey).not.toEqual(
      performanceReportOptions(client, 2, input).queryKey,
    );
  } finally {
    queries.clear();
  }
});
const explicit = () => entry("explicit").report as PerformanceReportData;
it("shows the returned lookback, summary, figures and episodes of the selected ticker", async () => {
  const report = explicit();
  render(<PerformanceReport report={report} />);
  const tiles = screen.getByRole("region", {
    name: "Lookback returns to 2025-06-30",
  });
  // Every returned digit by default; nothing rounded or scaled.
  expect(
    within(within(tiles).getByRole("region", { name: "Fytd" })).getByText(
      String(report.lookback.returns.fytd[0]),
    ),
  ).toBeTruthy();
  expect(within(tiles).getByText(/month 4, day 1/)).toBeTruthy();
  const summary = within(
    screen.getByRole("region", { name: "Summary" }),
  ).getByRole("table");
  expect(within(summary).getAllByRole("row")).toHaveLength(
    1 + report.summary.row_count,
  );
  for (const label of [
    "Cumulative returns by ticker",
    "Global Macro Composite drawdown",
    "Global Macro Composite annual returns",
    "Global Macro Composite rolling sharpe",
    "Global Macro Composite rolling volatility",
    "Global Macro Composite monthly returns heatmap",
  ])
    expect(screen.getByRole("img", { name: label })).toBeTruthy();
  const episodes = within(
    screen.getByRole("region", {
      name: "Global Macro Composite drawdown episodes",
    }),
  ).getByRole("table");
  expect(within(episodes).getAllByRole("row")).toHaveLength(
    1 + report.tickers[0]!.drawdownEpisodes.length,
  );
  await userEvent.click(screen.getByRole("combobox", { name: "Ticker" }));
  await userEvent.click(
    await screen.findByRole("option", { name: "Benchmark (benchmark)" }),
  );
  expect(screen.getByRole("img", { name: "Benchmark drawdown" })).toBeTruthy();
  expect(
    within(within(tiles).getByRole("region", { name: "Ytd" })).getByText(
      String(report.lookback.returns.ytd[1]),
    ),
  ).toBeTruthy();
});
it("reports a supplied colour domain that excludes a returned month and shows non-finite values as returned", () => {
  const report = explicit();
  const nan = {
    ...report,
    lookback: {
      ...report.lookback,
      returns: {
        ...report.lookback.returns,
        mtd: [Number.NaN, ...report.lookback.returns.mtd.slice(1)],
      },
    },
  };
  render(
    <PerformanceReport
      report={nan}
      monthlyDomain={{ palette: "diverging", domain: [-0.01, 0, 0.01] }}
      formatValue={(value) => value.toFixed(4)}
    />,
  );
  expect(
    screen.getByText(/colour domain \[-0.01, 0, 0.01\] excludes/),
  ).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "Mtd" })).getByText("NaN"),
  ).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "Qtd" })).getByText(
      report.lookback.returns.qtd[0]!.toFixed(4),
    ),
  ).toBeTruthy();
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<PerformanceReport report={null} />);
  expect(screen.getByRole("status").textContent).toBe("No performance report");
  rerender(<PerformanceReport report={null} loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Calculating performance…",
  );
  rerender(<PerformanceReport report={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
function Connected({ request }: { request: PerformanceReportRequest }) {
  const report = usePerformanceReport(request);
  return (
    <PerformanceReport
      report={report.data ?? null}
      loading={report.isFetching}
      error={report.error?.message}
    />
  );
}
it("renders the worker report through the hook, then the native error", async () => {
  const { request } = entry("defaults");
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        screen.getByRole("region", { name: /Lookback returns to/ }),
      ).toBeTruthy(),
    { timeout: budget(5000) },
  );
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected request={{ ...request, frequency: "fortnightly" }} />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
