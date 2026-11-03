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
  BrinsonReport,
  type BrinsonReportInputs,
  type BrinsonReportProps,
  type CarinoLinkedAttribution,
} from "@/components/finstack/portfolio/components/brinson-report/brinson-report";
import { brinsonLinkedOptions } from "@/hooks/portfolio/use-brinson-linked/use-brinson-linked";
import {
  PortfolioWorkbench,
  type PortfolioWorkbenchProps,
} from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./brinson/cases.json";
import workbench from "./workbench/cases.json";
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
const inputs = fixture.inputs as BrinsonReportInputs;
const result = fixture.result as CarinoLinkedAttribution;
const failures = Object.entries(fixture.errors);
function view(overrides: Partial<BrinsonReportProps> = {}) {
  return render(
    <BrinsonReport
      report={result}
      periodLabels={inputs.periodLabels}
      {...overrides}
    />,
  );
}
const region = (name: string) => within(screen.getByRole("region", { name }));
const shows = (scope: ReturnType<typeof region>, value: number) =>
  expect(scope.getAllByTitle(String(value)).length).toBeGreaterThan(0);

it(
  "the worker returns the native linked attribution of the host's periods",
  async () => {
    const linked = unwrap(
      await proxy().brinsonLinked({ periods: inputs.periods }),
    );
    // The fixture stores JSON, where Rust's -0.0 effects read as 0.
    expect(JSON.parse(JSON.stringify(linked))).toEqual(result);
  },
  budget(30000),
);
it.each(failures)(
  "the worker returns the native error of %s",
  async (_name, entry) => {
    const outcome = await proxy().brinsonLinked({
      periods: entry.request as BrinsonReportInputs["periods"],
    });
    expect(outcome.ok).toBe(false);
    if (outcome.ok) return;
    expect(outcome.error.message).toBe(entry.error.message);
    expect(outcome.error.kind).toBe(entry.error.kind);
  },
  budget(30000),
);
it("Rust names the failing period by its zero-based index", () => {
  expect(fixture.errors.weights.error.message).toContain("period 1");
  expect(fixture.errors.reordered.error.message).toContain("period 2");
});
it("shows the compounded returns as returned", () => {
  view();
  const compounded = region("Compounded returns");
  shows(compounded, result.portfolio_return_compounded);
  shows(compounded, result.benchmark_return_compounded);
  expect(compounded.getByText("Portfolio return compounded")).toBeTruthy();
});
it("shows every sector's linked effects and Rust's linked total of each effect", () => {
  view();
  const linked = region("Linked effects by sector");
  const table = within(
    linked.getByRole("table", { name: "Linked effects by sector" }),
  );
  const rows = table.getAllByRole("row");
  // Header, one row per returned sector in returned order, and the footer.
  expect(rows).toHaveLength(result.linked_sectors.length + 2);
  result.linked_sectors.forEach((sector, index) => {
    const row = within(rows[index + 1]!);
    expect(row.getByText(sector.sector)).toBeTruthy();
    for (const effect of [
      sector.allocation,
      sector.selection,
      sector.interaction,
      sector.total,
    ])
      shows(row, effect);
  });
  const footer = within(rows.at(-1)!);
  expect(footer.getByText("Linked total")).toBeTruthy();
  shows(footer, result.linked_allocation);
  shows(footer, result.linked_selection);
  shows(footer, result.linked_interaction);
  expect(
    linked.getByRole("img", { name: "Linked effect by sector" }),
  ).toBeTruthy();
});
it("shows every period's returns, sector effects and returned totals under the host's names", () => {
  view();
  result.periods.forEach((period, index) => {
    const name = inputs.periodLabels![index]!;
    const section = region(`${name} attribution`);
    shows(section, period.portfolio_return);
    shows(section, period.benchmark_return);
    const rows = within(
      section.getByRole("table", { name: `${name} effects by sector` }),
    ).getAllByRole("row");
    expect(rows).toHaveLength(period.sectors.length + 2);
    period.sectors.forEach((sector, at) => {
      const row = within(rows[at + 1]!);
      expect(row.getByText(sector.sector)).toBeTruthy();
      for (const effect of [
        sector.allocation,
        sector.selection,
        sector.interaction,
        sector.total,
      ])
        shows(row, effect);
    });
    const footer = within(rows.at(-1)!);
    expect(footer.getByText("Period total")).toBeTruthy();
    shows(footer, period.total_allocation);
    shows(footer, period.total_selection);
    shows(footer, period.total_interaction);
    shows(footer, period.total_excess_return);
  });
});
it("names each period by its position when the host supplies no names", () => {
  view({ periodLabels: undefined });
  expect(
    screen
      .getAllByRole("region")
      .map((node) => node.getAttribute("aria-label"))
      .filter((name) => name?.startsWith("Period")),
  ).toEqual([
    "Period 1 attribution",
    "Period 2 attribution",
    "Period 3 attribution",
  ]);
});
it("shows the native error, loading and no result", () => {
  const { message } = fixture.errors.weights.error;
  view({ report: null, error: message });
  expect(screen.getByRole("alert").textContent).toBe(message);
  cleanup();
  view({ report: null, loading: true });
  expect(screen.getByRole("status").textContent).toBe("Linking attribution…");
  cleanup();
  view({ report: null });
  expect(screen.getByRole("status").textContent).toBe("No Brinson attribution");
});
it("the hook is disabled without periods and keys on the whole request", () => {
  const client = {} as FinstackClient;
  expect(brinsonLinkedOptions(client, 1, null).enabled).toBe(false);
  expect(brinsonLinkedOptions(client, 1, inputs.periods).queryKey).toEqual([
    "finstack",
    1,
    "brinsonLinked",
    inputs.periods,
  ]);
});
const props: PortfolioWorkbenchProps = {
  defaultSpecJson: workbench.specJson,
  marketJson: JSON.stringify(pricingMarket),
  asOf: workbench.request.asOf,
  brinson: inputs,
};
function renderWorkbench(overrides: Partial<PortfolioWorkbenchProps> = {}) {
  return render(
    <FinstackQueryProvider>
      <PortfolioWorkbench {...props} {...overrides} />
    </FinstackQueryProvider>,
  );
}
it("offers the Brinson tab only when the host supplies periods", () => {
  renderWorkbench({ brinson: undefined });
  expect(screen.queryByRole("tab", { name: "Brinson" })).toBeNull();
  cleanup();
  renderWorkbench();
  expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual([
    "Positions",
    "Risk",
    "Brinson",
    "Margin",
    "Book",
  ]);
  expect(mock.calls).not.toContain("brinsonLinked");
});
it(
  "links the host's periods natively once when the tab opens",
  async () => {
    const user = userEvent.setup();
    renderWorkbench();
    await user.click(screen.getByRole("tab", { name: "Brinson" }));
    expect(
      (
        await screen.findAllByTitle(
          String(result.portfolio_return_compounded),
          {},
          { timeout: budget(10000) },
        )
      ).length,
    ).toBeGreaterThan(0);
    expect(
      screen.getByRole("region", {
        name: `${inputs.periodLabels![2]} attribution`,
      }),
    ).toBeTruthy();
    expect(screen.queryAllByRole("alert")).toEqual([]);
    expect(mock.calls.filter((name) => name === "brinsonLinked")).toHaveLength(
      1,
    );
  },
  budget(30000),
);
