// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
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
  PortfolioAttributionReport,
  type PortfolioAttributionData,
} from "@/components/finstack/portfolio/components/portfolio-attribution-report/portfolio-attribution-report";
import {
  PortfolioWorkbench,
  type PortfolioWorkbenchProps,
} from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import { attributePortfolioPnlOptions } from "@/hooks/portfolio/use-attribute-portfolio-pnl/use-attribute-portfolio-pnl";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { serializeHost } from "../../src/codec.mjs";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./attribution/cases.json";
import pricingMarket from "../valuations/instruments/pricing-market.json";
const mock = vi.hoisted(() => ({
  createClient: vi.fn(),
  calls: [] as string[],
}));
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
const marketJson = JSON.stringify(pricingMarket);
const { specJson, asOfT0, reconciliationTolerance, closing } = fixture;
type Scenario = keyof typeof closing;
const entry = (scenario: Scenario, name: string) =>
  fixture.cases.find(
    (candidate) => candidate.scenario === scenario && candidate.name === name,
  )!;
const report = (name: string) =>
  entry("usd-ois-50bp", name).report as unknown as PortfolioAttributionData;
const requestFor = (scenario: Scenario, name: string) => ({
  specJson,
  marketT0Json: marketJson,
  ...closing[scenario],
  asOfT0,
  method: entry(scenario, name).method as any,
  reconciliationTolerance,
});
const rowsOf = (table: HTMLElement) =>
  (within(table).getAllByRole("row") as HTMLTableRowElement[]).slice(1);
// The fixture is regenerated and compared by `mise run ui-fixtures --check`
// (tests/portfolio/attribution/generate.mjs).
it("pins the JSON inputs the attribution fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/portfolio/attribution/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it("records every Rust method on both closing markets", () => {
  expect(
    fixture.cases.map(({ scenario, name, ok }) => [scenario, name, ok]),
  ).toEqual([
    ["usd-ois-50bp", "parallel", true],
    ["usd-ois-50bp", "waterfall", true],
    ["usd-ois-50bp", "metrics_based", true],
    ["usd-ois-50bp", "taylor", true],
    ["roll-1m-25bp", "parallel", false],
    ["roll-1m-25bp", "waterfall", false],
    ["roll-1m-25bp", "metrics_based", false],
    ["roll-1m-25bp", "taylor", false],
  ]);
  // Rust's verdict at the host's tolerance, as returned.
  for (const name of ["parallel", "waterfall", "metrics_based", "taylor"])
    expect(report(name).reconciliation.is_reconciled).toBe(true);
});
it(
  "returns through the real worker every native result the generator recorded",
  async () => {
    for (const { scenario, name, ok, ...recorded } of fixture.cases) {
      const result = await proxy().attributePortfolioPnl(
        requestFor(scenario as Scenario, name),
      );
      expect(result.ok, `${scenario} ${name}`).toBe(ok);
      if (result.ok)
        expect(serializeHost(result.value)).toBe(
          serializeHost((recorded as { report: unknown }).report),
        );
      else {
        expect(result.error.kind).toBe((recorded as { kind: string }).kind);
        expect(result.error.message).toBe(
          (recorded as { message: string }).message,
        );
      }
    }
  },
  budget(30000),
);
it("keys the attribution on the worker session and the complete snapshotted request", () => {
  const supplied = requestFor("usd-ois-50bp", "taylor");
  const options = attributePortfolioPnlOptions(null, 3, supplied);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "attributePortfolioPnl",
    supplied,
  ]);
  expect(options.queryKey[3]).not.toBe(supplied);
});
it("shows the book total, Rust's reconciliation and every Rust bucket as returned", () => {
  const data = report("metrics_based");
  render(<PortfolioAttributionReport report={data} />);
  const view = screen.getByRole("region", { name: "Portfolio attribution" });
  expect(
    within(view).getAllByTitle(data.attribution.total_pnl.amount).length,
  ).toBeGreaterThan(0);
  const reconciliation = within(view).getByLabelText("Reconciliation");
  expect(reconciliation.textContent).toContain("Reconciled");
  const factors = within(view).getByRole("region", { name: "Book factors" });
  expect(
    rowsOf(within(factors).getByRole("table")).map(
      (row) => row.cells[0]!.textContent,
    ),
  ).toEqual(data.buckets.map((bucket) => labelFor(bucket.field)));
  for (const bucket of data.buckets)
    expect(
      within(factors).getAllByTitle(bucket.amount.amount).length,
    ).toBeGreaterThan(0);
  expect(
    within(view).getByRole("region", { name: "Book explain" }).textContent,
  ).toContain(data.explain);
  expect(screen.queryAllByRole("alert")).toEqual([]);
});
it(
  "returns through the real worker Rust's verdict at an exact host tolerance",
  async () => {
    const { exact } = fixture;
    const result = await proxy().attributePortfolioPnl({
      ...requestFor("usd-ois-50bp", "metrics_based"),
      reconciliationTolerance: exact.reconciliationTolerance,
    });
    expect(result.ok).toBe(true);
    if (result.ok)
      expect(serializeHost(result.value)).toBe(serializeHost(exact.report));
  },
  budget(30000),
);
it("says when Rust does not reconcile the book, and which positions' residuals fail", async () => {
  const data = fixture.exact.report as unknown as PortfolioAttributionData;
  expect(data.reconciliation.is_reconciled).toBe(false);
  render(<PortfolioAttributionReport report={data} />);
  const view = screen.getByRole("region", { name: "Portfolio attribution" });
  const reconciliation = within(view).getByLabelText("Reconciliation");
  expect(within(reconciliation).getByText("Not reconciled")).toBeTruthy();
  expect(within(reconciliation).queryByText("Reconciled")).toBeNull();
  const failing = data.positions.findIndex(
    (position) => !position.residualWithinTolerance,
  );
  expect(failing).toBeGreaterThanOrEqual(0);
  if (failing > 0)
    await userEvent.click(
      rowsOf(
        within(screen.getByRole("region", { name: "Positions" })).getByRole(
          "table",
        ),
      )[failing]!.cells[0]!,
    );
  const detail = screen.getByRole("region", {
    name: `Position ${data.positions[failing]!.positionId}`,
  });
  expect(
    within(within(detail).getByLabelText("Residual")).getByText(
      "Residual outside tolerance",
    ),
  ).toBeTruthy();
});
it("draws each position by the book's buckets from its own Rust buckets and drills into one", async () => {
  const data = report("waterfall");
  render(<PortfolioAttributionReport report={data} />);
  const positions = screen.getByRole("region", { name: "Positions" });
  const table = within(positions).getByRole("table");
  const header = within(table).getAllByRole("columnheader");
  expect(header.map((cell) => cell.textContent)).toEqual([
    "Position",
    "Currency",
    "Total P&L",
    ...data.buckets.map((bucket) => labelFor(bucket.field)),
  ]);
  const rows = rowsOf(table);
  expect(rows.map((row) => row.cells[0]!.textContent)).toEqual(
    data.positions.map((position) => position.positionId),
  );
  for (const [index, position] of data.positions.entries()) {
    const row = rows[index]!;
    expect(row.cells[1]!.textContent).toBe(
      position.attribution.total_pnl.currency,
    );
    expect(
      within(row).getAllByTitle(position.attribution.total_pnl.amount).length,
    ).toBeGreaterThan(0);
    for (const [column, bucket] of data.buckets.entries())
      expect(
        row.cells[3 + column]!.querySelector("[title]")?.getAttribute("title"),
      ).toBe(
        position.buckets.find((own) => own.field === bucket.field)!.amount
          .amount,
      );
  }
  // The first returned position starts selected; activating a row selects it.
  const [first, second] = data.positions;
  expect(
    screen.getByRole("region", { name: `Position ${first!.positionId}` }),
  ).toBeTruthy();
  await userEvent.click(rows[1]!.cells[0]!);
  const detail = screen.getByRole("region", {
    name: `Position ${second!.positionId}`,
  });
  expect(
    within(detail).getByRole("region", { name: "Explain" }).textContent,
  ).toContain(second!.explain);
  expect(
    within(detail).getByRole("region", { name: "Waterfall" }),
  ).toBeTruthy();
});
it("shows a native error as returned", () => {
  const failed = entry("roll-1m-25bp", "parallel") as { message: string };
  render(<PortfolioAttributionReport report={null} error={failed.message} />);
  expect(screen.getByRole("alert").textContent).toBe(failed.message);
});
const props: PortfolioWorkbenchProps = {
  defaultSpecJson: specJson,
  marketJson,
  asOf: asOfT0,
  attribution: { ...closing["usd-ois-50bp"], reconciliationTolerance },
};
function renderWorkbench(overrides: Partial<PortfolioWorkbenchProps> = {}) {
  return render(
    <FinstackQueryProvider>
      <PortfolioWorkbench {...props} {...overrides} />
    </FinstackQueryProvider>,
  );
}
it("offers the Attribution tab only when the host supplies a closing market", () => {
  renderWorkbench({ attribution: undefined });
  expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual([
    "Positions",
    "Risk",
    "Margin",
    "Book",
  ]);
  cleanup();
  renderWorkbench();
  expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual([
    "Positions",
    "Risk",
    "Attribution",
    "Margin",
    "Book",
  ]);
});
it(
  "attributes the book under each method the user picks, with no method preselected",
  async () => {
    const user = userEvent.setup();
    renderWorkbench();
    await user.click(screen.getByRole("tab", { name: "Attribution" }));
    expect(
      screen.getByText("Choose a method; Rust requires one."),
    ).toBeTruthy();
    expect(mock.calls).not.toContain("attributePortfolioPnl");
    for (const name of ["parallel", "waterfall", "metrics_based", "taylor"]) {
      await user.click(await screen.findByRole("radio", { name }));
      const view = await screen.findByRole(
        "region",
        { name: "Portfolio attribution" },
        { timeout: budget(10000) },
      );
      expect(
        within(view).getByRole("region", { name: "Book explain" }).textContent,
      ).toContain(report(name).explain);
    }
    expect(
      mock.calls.filter((call) => call === "attributePortfolioPnl"),
    ).toHaveLength(4);
    expect(screen.queryAllByRole("alert")).toEqual([]);
  },
  budget(40000),
);
it(
  "shows Rust's missing-fixing error for a closing market a month on, without hiding the method",
  async () => {
    const user = userEvent.setup();
    renderWorkbench({
      attribution: { ...closing["roll-1m-25bp"], reconciliationTolerance },
    });
    await user.click(screen.getByRole("tab", { name: "Attribution" }));
    await user.click(await screen.findByRole("radio", { name: "parallel" }));
    const failed = entry("roll-1m-25bp", "parallel") as { message: string };
    expect(
      (await screen.findByRole("alert", {}, { timeout: budget(10000) }))
        .textContent,
    ).toBe(failed.message);
    expect(screen.getAllByRole("radio")).toHaveLength(4);
  },
  budget(20000),
);
