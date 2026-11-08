// @vitest-environment jsdom
import type { BenchmarkReportData } from "@/components/finstack/analytics/lib/analytics-report-types";
import { createRequire } from "node:module";
import { afterEach, expect, it, vi } from "vitest";
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
  benchmarkReportOptions,
  useBenchmarkReport,
  type BenchmarkReport as Report,
  type BenchmarkReportRequest,
} from "@/hooks/analytics/use-benchmark-report/use-benchmark-report";
import { BenchmarkReport } from "@/components/finstack/analytics/components/benchmark-report/benchmark-report";
import { unwrap } from "@/workers/finstack-contract";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import { fixtureNumber } from "./fixture-numbers";
import tearSheet from "./benchmark.json";
import invariantsCase from "./benchmark-invariants.json";
// The shared Rust, Python and WASM analytics parity fixture.
import data from "../../../finstack-quant/analytics/tests/data/api_invariants_data.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
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
type Resources = Record<"performanceConstructed" | "performanceFreed", number>;
const harness = useWorkerHarness(["benchmarkReport"]);
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const cases = { tearSheet, invariants: invariantsCase };
const entry = (name: keyof typeof cases) => ({
  request: cases[name].request as BenchmarkReportRequest,
  report: {
    ...cases[name].report,
    informationRatio: cases[name].report.informationRatio.map(fixtureNumber),
  } as unknown as Report,
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
it("matches shared analytics invariants and uses the explicit rolling window", () => {
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
  // The tear-sheet case explicitly selects a 63-observation window.
  const [fund] = entry("tearSheet").report.tickers;
  expect(fund!.rollingGreeks.alphas).toHaveLength(
    fund!.activeDates.length - 62,
  );
  expect(entry("tearSheet").report.benchmarkIdx).toBe(1);
});
it("aligns relative series with each ticker's exact paired benchmark dates on a ragged panel", async () => {
  const { request } = entry("invariants");
  const ragged: BenchmarkReportRequest = {
    ...request,
    factors: null,
    panel: {
      ...request.panel,
      // A leading gap is null on the wire.
      series: [
        request.panel.series[0]!.map((value, i) => (i < 3 ? null : value)),
        request.panel.series[1]!,
      ],
    },
  };
  const report = unwrap(await harness.proxy.benchmarkReport(ragged));
  const [fund, bench] = report.tickers;
  expect(fund!.activeDates).toHaveLength(request.panel.dates.length - 4);
  expect(fund!.outperformance).toHaveLength(fund!.activeDates.length);
  expect(fund!.drawdownDifference).toHaveLength(fund!.activeDates.length);
  expect(bench!.outperformance).toHaveLength(request.panel.dates.length - 1);
  expect(fund!.multiFactor).toBeNull();
});
it("returns a ticker's rejected regression as its native error and keeps the rest of the report", async () => {
  const { request } = entry("invariants");
  const report = unwrap(
    await harness.proxy.benchmarkReport({
      ...request,
      factors: { ...request.factors!, riskFreeRate: -1 },
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
    panel: { ...request.panel, benchmark_ticker: "Missing" },
  });
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) {
    expect(unknown.error.kind).toBe("validation");
    expect(unknown.error.message).toContain("Missing");
  }
});
it("builds a price panel with the native constructor", async () => {
  const { request } = entry("invariants");
  const prices = request.panel.series.map((row) =>
    row.reduce<number[]>(
      (levels, r) => [...levels, levels.at(-1)! * (1 + r!)],
      [100],
    ),
  );
  const panel = {
    dates: request.panel.dates,
    series: prices,
    kind: "prices" as const,
    ticker_names: ["TARGET", "BENCH"],
    benchmark_ticker: "BENCH",
    frequency: "daily" as const,
  };
  const report = unwrap(
    await harness.proxy.benchmarkReport({ panel, riskFreeRate: 0, window: 5 }),
  );
  const direct = new native.Performance(panel);
  try {
    expect(report.greeks).toEqual(direct.greeks(0));
    expect(report.trackingError).toEqual(Array.from(direct.trackingError()));
  } finally {
    direct.free();
  }
});
it("keys the query on the worker session and the complete snapshotted request", () => {
  const { request } = entry("invariants");
  const options = benchmarkReportOptions(null, 2, request);
  expect(options.queryKey).toEqual([
    "finstack",
    2,
    "benchmarkReport",
    JSON.stringify(request),
  ]);
  expect(benchmarkReportOptions(null, 2, null).enabled).toBe(false);
});
it("isolates non-finite inputs and sessions while reusing identical requests", async () => {
  const { request, report } = entry("invariants");
  const call = vi.fn(async () => report);
  const client = { call } as unknown as FinstackClient;
  const queries = new QueryClient();
  try {
    for (const riskFreeRate of [0, Number.NaN, Infinity, -Infinity]) {
      const input = { ...request, riskFreeRate };
      const options = benchmarkReportOptions(client, 2, input);
      await queries.query(options);
      await queries.query(
        benchmarkReportOptions(client, 2, structuredClone(input)),
      );
    }
    expect(call).toHaveBeenCalledTimes(4);
    await queries.query(
      benchmarkReportOptions(client, 3, { ...request, riskFreeRate: 0 }),
    );
    expect(call).toHaveBeenCalledTimes(5);
  } finally {
    queries.clear();
  }
});
it("sends the original numeric snapshot after the caller mutates its request", async () => {
  const { request, report } = entry("invariants");
  const input = {
    ...request,
    riskFreeRate: Number.NaN,
    panel: {
      ...request.panel,
      series: request.panel.series.map((row) => [...row]),
    },
  };
  input.panel.series[0]![0] = Infinity;
  const expected = structuredClone(input);
  const call = vi.fn(async () => report);
  const client = { call } as unknown as FinstackClient;
  const options = benchmarkReportOptions(client, 2, input);
  const key = options.queryKey[3];
  input.riskFreeRate = 0;
  input.panel.series[0]![0] = 0;
  const queries = new QueryClient();
  try {
    await queries.query(options);
    expect(call).toHaveBeenCalledWith("benchmarkReport", expected);
    expect(options.queryKey[3]).toBe(key);
    expect(options.queryKey).not.toEqual(
      benchmarkReportOptions(client, 2, input).queryKey,
    );
  } finally {
    queries.clear();
  }
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
      <Connected
        request={{
          ...request,
          // Not a Rust PeriodKind: the native error surfaces.
          panel: { ...request.panel, frequency: "fortnightly" as never },
        }}
      />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
