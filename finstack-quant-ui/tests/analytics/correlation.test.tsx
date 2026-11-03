// @vitest-environment jsdom
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  CorrelationHeatmap,
  type CorrelationHeatmapProps,
} from "@/components/finstack/analytics/components/correlation-heatmap/correlation-heatmap";
import {
  correlationMatrixOptions,
  type CorrelationMatrixReport,
  type CorrelationMatrixRequest,
} from "@/hooks/analytics/use-correlation-matrix/use-correlation-matrix";
import {
  PortfolioWorkbench,
  type PortfolioWorkbenchProps,
} from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./correlation/cases.json";
import { failures, panel } from "./correlation/panels.mjs";
import workbench from "../portfolio/workbench/cases.json";
import pricingMarket from "../valuations/instruments/pricing-market.json";

const mock = vi.hoisted(() => ({
  createClient: vi.fn(),
  calls: [] as string[],
}));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
// Figures are drawn by the shared chart in the browser gates.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
let harness: Awaited<ReturnType<typeof startWorker>>;
const proxy = () => harness.proxy as unknown as WorkerApi;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.calls.length = 0;
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) => {
      mock.calls.push(method);
      return unwrap(await (proxy() as any)[method](request));
    }),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const request = fixture.request as CorrelationMatrixRequest;
const result = fixture.result as CorrelationMatrixReport;
function view(overrides: Partial<CorrelationHeatmapProps> = {}) {
  return render(<CorrelationHeatmap report={result} {...overrides} />);
}

it("stores the panel panels.mjs builds", () => {
  expect(request).toEqual(panel);
});
it(
  "the worker returns the native matrix and Rust's repaired flag",
  async () => {
    const matrix = unwrap(await proxy().correlationMatrix(request));
    expect(JSON.parse(JSON.stringify(matrix))).toEqual(result);
    expect(result.repaired).toBe(false);
  },
  budget(30000),
);
it.each(Object.keys(failures) as (keyof typeof failures)[])(
  "the worker returns the native error of %s",
  async (name) => {
    const outcome = await proxy().correlationMatrix(
      failures[name] as CorrelationMatrixRequest,
    );
    expect(outcome.ok).toBe(false);
    if (outcome.ok) return;
    expect(outcome.error.message).toBe(fixture.errors[name].message);
    expect(outcome.error.kind).toBe(fixture.errors[name].kind);
  },
  budget(30000),
);
it("draws every returned coefficient in Rust's ticker order", () => {
  view();
  const section = within(
    screen.getByRole("region", { name: "Return correlation" }),
  );
  expect(
    section.getByRole("img", { name: "Correlation heatmap" }),
  ).toBeTruthy();
  const rows = within(
    section.getByRole("table", { name: "Correlation matrix" }),
  ).getAllByRole("row");
  expect(rows).toHaveLength(result.tickerNames.length + 1);
  expect(
    within(rows[0]!)
      .getAllByRole("columnheader")
      .map((cell) => cell.textContent),
  ).toEqual(["Ticker", ...result.tickerNames]);
  result.matrix.forEach((values, i) => {
    const row = within(rows[i + 1]!);
    expect(row.getByText(result.tickerNames[i]!)).toBeTruthy();
    values.forEach((value) =>
      expect(row.getAllByTitle(String(value)).length).toBeGreaterThan(0),
    );
  });
});
it("stamps the matrix only when Rust says it repaired it", () => {
  view();
  expect(screen.queryByRole("note", { name: "Repaired" })).toBeNull();
  expect(screen.getByText(/did not need to repair/)).toBeTruthy();
  cleanup();
  // The same native coefficients with Rust's flag set by hand. No panel
  // reaches Rust's Higham repair through Performance today: its pairwise
  // estimate has always been a valid correlation matrix, so no native fixture
  // has repaired: true and this branch is driven by the supplied boolean.
  view({ report: { ...result, repaired: true } });
  expect(screen.getByRole("note", { name: "Repaired" }).textContent).toBe(
    "Repaired",
  );
  expect(screen.getByText(/nearest valid one \(Higham\)/)).toBeTruthy();
});
it("shows the native error, loading and no result", () => {
  const { message } = fixture.errors.noOverlap;
  view({ report: null, error: message });
  expect(screen.getByRole("alert").textContent).toBe(message);
  cleanup();
  view({ report: null, loading: true });
  expect(screen.getByRole("status").textContent).toBe(
    "Estimating correlations…",
  );
  cleanup();
  view({ report: null });
  expect(screen.getByRole("status").textContent).toBe("No correlation matrix");
});
it("the hook is disabled without a panel and keys on the whole panel", () => {
  const client = {} as FinstackClient;
  expect(correlationMatrixOptions(client, 1, null).enabled).toBe(false);
  expect(correlationMatrixOptions(client, 1, request).queryKey).toEqual([
    "finstack",
    1,
    "correlationMatrix",
    JSON.stringify(request),
  ]);
});
const props: PortfolioWorkbenchProps = {
  defaultSpecJson: workbench.specJson,
  marketJson: JSON.stringify(pricingMarket),
  asOf: workbench.request.asOf,
  correlation: request,
};
function renderWorkbench(overrides: Partial<PortfolioWorkbenchProps> = {}) {
  return render(
    <FinstackQueryProvider>
      <PortfolioWorkbench {...props} {...overrides} />
    </FinstackQueryProvider>,
  );
}
it(
  "correlates the host's panel once when the Risk tab opens",
  async () => {
    const user = userEvent.setup();
    renderWorkbench();
    expect(mock.calls).not.toContain("correlationMatrix");
    await user.click(screen.getByRole("tab", { name: "Risk" }));
    const table = await screen.findByRole(
      "table",
      { name: "Correlation matrix" },
      { timeout: budget(10000) },
    );
    expect(
      within(table).getAllByTitle(String(result.matrix[0]![2])).length,
    ).toBeGreaterThan(0);
    expect(
      mock.calls.filter((name) => name === "correlationMatrix"),
    ).toHaveLength(1);
  },
  budget(30000),
);
it(
  "shows no correlation when the host supplies no panel",
  async () => {
    const user = userEvent.setup();
    renderWorkbench({ correlation: undefined });
    await user.click(screen.getByRole("tab", { name: "Risk" }));
    await screen.findByRole(
      "region",
      { name: "Primitive exposures" },
      { timeout: budget(10000) },
    );
    expect(
      screen.queryByRole("region", { name: "Return correlation" }),
    ).toBeNull();
    expect(mock.calls).not.toContain("correlationMatrix");
  },
  budget(30000),
);
