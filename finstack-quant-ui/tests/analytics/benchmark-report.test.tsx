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
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  benchmarkReportOptions,
  useBenchmarkReport,
  type BenchmarkReport as Report,
  type BenchmarkReportRequest,
} from "@/hooks/analytics/use-benchmark-report/use-benchmark-report";
import {
  BenchmarkReport,
  type BenchmarkReportData,
} from "@/components/finstack/analytics/components/benchmark-report/benchmark-report";
import { unwrap } from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import tearSheet from "./benchmark.json";
import invariantsCase from "./benchmark-invariants.json";
// The shared Rust, Python and WASM analytics parity fixture.
import data from "../../../finstack-quant/analytics/tests/data/api_invariants_data.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
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
type Resources = Record<"performanceConstructed" | "performanceFreed", number>;
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
      if (method === "benchmarkReport")
        return unwrap(
          await harness.proxy.benchmarkReport(
            request as BenchmarkReportRequest,
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
const cases = { tearSheet, invariants: invariantsCase };
const entry = (name: keyof typeof cases) => ({
  request: cases[name].request as BenchmarkReportRequest,
  report: cases[name].report as unknown as Report,
});
// Reports are regenerated and compared by `mise run ui-fixtures --check`
// (tests/analytics/generate.mjs).
it.each(["tearSheet", "invariants"] as const)(
  "returns the %s native benchmark report through the real worker and frees the handle",
  async (name) => {
    const { request, report } = entry(name);
    const before = await resources();
    expect(unwrap(await harness.proxy.benchmarkReport(request))).toEqual(
      report,
    );
    const after = await resources();
    expect(after.performanceConstructed - before.performanceConstructed).toBe(
      1,
    );
    expect(after.performanceFreed - before.performanceFreed).toBe(1);
  },
  budget(15000),
);
it("matches the shared analytics invariants and leaves the default window to Rust", () => {
  const target = entry("invariants").report.tickers[0]!;
  expect(target.multiFactor?.ok).toBe(true);
  if (!target.multiFactor?.ok) return;
  const multi = target.multiFactor.value;
  const expected = data.expected.multi_factor_greeks;
  for (const key of [
    "alpha",
    "r_squared",
    "adjusted_r_squared",
    "residual_vol",
  ] as const)
    expect(multi[key]).toBeCloseTo(expected[key], 12);
  multi.betas.forEach((beta, i) =>
    expect(beta).toBeCloseTo(expected.betas[i], 12),
  );
  target.rollingGreeks.alphas.forEach((alpha, i) =>
    expect(alpha).toBeCloseTo(data.expected.rolling_greeks.alphas[i], 12),
  );
  expect(target.rollingGreeks.betas).toHaveLength(
    data.expected.rolling_greeks.betas.length,
  );
  // The tear-sheet case omits the window: Rust's 63 applies.
  const [fund] = entry("tearSheet").report.tickers;
  expect(fund!.rollingGreeks.alphas).toHaveLength(
    fund!.activeDates.length - 62,
  );
  expect(entry("tearSheet").report.benchmarkIdx).toBe(1);
});
it("aligns relative series with each ticker's own active dates on a ragged panel", async () => {
  const { request } = entry("invariants");
  const ragged: BenchmarkReportRequest = {
    ...request,
    factors: null,
    values: [
      request.values[0]!.map((value, i) => (i < 3 ? Number.NaN : value)),
      request.values[1]!,
    ],
  };
  const report = unwrap(await harness.proxy.benchmarkReport(ragged));
  const [fund, bench] = report.tickers;
  expect(fund!.activeDates).toHaveLength(request.dates.length - 3);
  expect(fund!.outperformance).toHaveLength(fund!.activeDates.length);
  expect(fund!.drawdownDifference).toHaveLength(fund!.activeDates.length);
  expect(bench!.outperformance).toHaveLength(request.dates.length);
  expect(fund!.multiFactor).toBeNull();
});
it("returns a ticker's rejected regression as its native error and keeps the rest of the report", async () => {
  const { request } = entry("invariants");
  const report = unwrap(
    await harness.proxy.benchmarkReport({
      ...request,
      factors: { ...request.factors!, riskFreeRate: 0.02 },
    }),
  );
  for (const ticker of report.tickers) {
    expect(ticker.multiFactor?.ok).toBe(false);
    if (ticker.multiFactor && !ticker.multiFactor.ok)
      expect(ticker.multiFactor.error.message).toBeTruthy();
  }
  expect(report.greeks).toEqual(entry("invariants").report.greeks);
});
it("returns the native error and still frees the panel when a later call fails", async () => {
  const { request } = entry("invariants");
  const before = await resources();
  const failed = await harness.proxy.benchmarkReport({ ...request, window: 0 });
  expect(failed.ok).toBe(false);
  if (!failed.ok)
    expect(failed.error.message).toContain("rolling window must be at least 1");
  const after = await resources();
  expect(after.performanceConstructed - before.performanceConstructed).toBe(1);
  expect(after.performanceFreed - before.performanceFreed).toBe(1);
  const unknown = await harness.proxy.benchmarkReport({
    ...request,
    benchmarkTicker: "Missing",
  });
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) expect(unknown.error.kind).toBe("not_found");
});
it("builds a price panel with the native constructor", async () => {
  const { request } = entry("invariants");
  const prices = request.values.map((row) =>
    row.reduce<number[]>(
      (levels, r) => [...levels, levels.at(-1)! * (1 + r)],
      [100],
    ),
  );
  const dates = ["2024-12-31", ...request.dates];
  const report = unwrap(
    await harness.proxy.benchmarkReport({
      dates,
      values: prices,
      kind: "prices",
      tickerNames: ["TARGET", "BENCH"],
      benchmarkTicker: "BENCH",
    }),
  );
  const direct = new native.Performance(
    dates,
    prices,
    ["TARGET", "BENCH"],
    "BENCH",
  );
  try {
    expect(report.greeks).toEqual(direct.greeks());
    expect(report.trackingError).toEqual(Array.from(direct.trackingError()));
  } finally {
    direct.free();
  }
});
it("keys the query on the worker session and the complete snapshotted request", () => {
  const { request } = entry("invariants");
  const options = benchmarkReportOptions(null, 2, request);
  expect(options.queryKey).toEqual(["finstack", 2, "benchmarkReport", request]);
  expect(options.queryKey[3]).not.toBe(request);
  expect(benchmarkReportOptions(null, 2, null).enabled).toBe(false);
});
const gallery = () => entry("tearSheet").report as BenchmarkReportData;
it("shows the first non-benchmark fund's returned measures, figures and multi-factor regression", async () => {
  const report = gallery();
  render(<BenchmarkReport report={report} />);
  const fund = "Global Macro Composite";
  const regression = within(
    screen.getByRole("region", {
      name: `${fund} regression on Benchmark`,
    }),
  ).getByRole("table");
  // Every returned digit by default; nothing rounded or scaled.
  expect(
    within(regression).getByText(String(report.greeks[0]!.alpha)),
  ).toBeTruthy();
  const relative = within(
    screen.getByRole("region", { name: `${fund} relative measures` }),
  ).getByRole("table");
  expect(within(relative).getAllByRole("row")).toHaveLength(1 + 8);
  expect(
    within(relative).getByText(String(report.trackingError[0])),
  ).toBeTruthy();
  for (const label of [
    `${fund} cumulative outperformance`,
    `${fund} drawdown difference`,
    `${fund} rolling alpha`,
    `${fund} rolling beta`,
  ])
    expect(screen.getByRole("img", { name: label })).toBeTruthy();
  const multi = within(
    screen.getByRole("region", { name: `${fund} multi-factor regression` }),
  ).getByRole("table");
  expect(within(multi).getByText("Beta · Carry")).toBeTruthy();
  await userEvent.click(screen.getByRole("combobox", { name: "Fund" }));
  await userEvent.click(
    await screen.findByRole("option", { name: "Benchmark (benchmark)" }),
  );
  expect(
    screen.getByRole("img", { name: "Benchmark rolling beta" }),
  ).toBeTruthy();
});
it("shows non-finite values as returned and a ticker's native regression error", () => {
  const report = gallery();
  const nan: BenchmarkReportData = {
    ...report,
    treynor: [Number.NaN, ...report.treynor.slice(1)],
    tickers: [
      {
        ...report.tickers[0]!,
        multiFactor: {
          ok: false,
          error: { message: "native regression failure" },
        },
      },
      ...report.tickers.slice(1),
    ],
  };
  render(
    <BenchmarkReport report={nan} formatValue={(value) => value.toFixed(4)} />,
  );
  const relative = screen.getByRole("region", {
    name: "Global Macro Composite relative measures",
  });
  expect(within(relative).getByText("NaN")).toBeTruthy();
  expect(
    within(relative).getByText(report.trackingError[0]!.toFixed(4)),
  ).toBeTruthy();
  // The observation count is an integer shown as returned, not host-formatted.
  const regression = screen.getByRole("region", {
    name: "Global Macro Composite regression on Benchmark",
  });
  expect(within(regression).getByText("756")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toBe(
    "native regression failure",
  );
});
it("omits the multi-factor section without a factor panel and honours initialTicker", () => {
  const report = gallery();
  render(
    <BenchmarkReport
      report={{
        ...report,
        factorNames: null,
        tickers: report.tickers.map((t) => ({ ...t, multiFactor: null })),
      }}
      initialTicker={1}
    />,
  );
  expect(screen.queryByRole("region", { name: /multi-factor/ })).toBeNull();
  expect(
    screen.getByRole("region", { name: "Benchmark relative measures" }),
  ).toBeTruthy();
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<BenchmarkReport report={null} />);
  expect(screen.getByRole("status").textContent).toBe("No benchmark report");
  rerender(<BenchmarkReport report={null} loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Calculating benchmark measures…",
  );
  rerender(<BenchmarkReport report={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
function Connected({ request }: { request: BenchmarkReportRequest }) {
  const report = useBenchmarkReport(request);
  return (
    <BenchmarkReport
      report={report.data ?? null}
      loading={report.isFetching}
      error={report.error?.message}
    />
  );
}
it("renders the worker report through the hook, then the native error", async () => {
  const { request } = entry("invariants");
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        screen.getByRole("region", { name: "TARGET relative measures" }),
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
