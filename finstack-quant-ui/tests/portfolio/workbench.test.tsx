// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
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
  PortfolioWorkbench,
  type PortfolioWorkbenchProps,
} from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import fixture from "./workbench/cases.json";
import pricingMarket from "../valuations/instruments/pricing-market.json";
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
const harness = useWorkerHarness("any");
const proxy = () => harness.proxy as unknown as WorkerApi;
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const marketJson = JSON.stringify(pricingMarket);
const { specJson, request } = fixture;
const props: PortfolioWorkbenchProps = {
  defaultSpecJson: specJson,
  marketJson,
  asOf: request.asOf,
  metrics: request.metrics,
  seriesBase: request.seriesBase,
  scenarioJsons: request.scenarioJsons,
};
function renderWorkbench(overrides: Partial<PortfolioWorkbenchProps> = {}) {
  return render(
    <FinstackQueryProvider>
      <PortfolioWorkbench {...props} {...overrides} />
    </FinstackQueryProvider>,
  );
}
const tab = (name: string) => screen.getByRole("tab", { name });
const rowsOf = (table: HTMLElement) =>
  (within(table).getAllByRole("row") as HTMLTableRowElement[]).slice(1);
// The fixture is regenerated and compared by `mise run ui-fixtures --check`
// (tests/portfolio/workbench/generate.mjs).
it(
  "returns through the real worker every native result the generator recorded",
  async () => {
    const report = unwrap(
      await proxy().valuePortfolio({
        specJson,
        marketJson,
        metrics: request.metrics,
      }),
    );
    expect(serializeHost(report)).toBe(serializeHost(fixture.report));
    expect(
      unwrap(
        await proxy().portfolioMetricsSeries({
          metrics: report.metrics,
          base: request.seriesBase,
        }),
      ),
    ).toEqual(fixture.series.entries);
    expect(
      serializeHost(
        unwrap(
          await proxy().scenarioPnlBatch({
            specJson,
            marketJson,
            scenarioJsons: request.scenarioJsons,
          }),
        ),
      ),
    ).toBe(serializeHost(fixture.batch));
    expect(
      JSON.parse(
        JSON.stringify(
          unwrap(
            await proxy().portfolioMargin({
              specJson,
              marketJson,
              asOf: request.asOf,
            }),
          ),
        ),
      ),
    ).toEqual(fixture.margin);
  },
  budget(20000),
);
it(
  "opens on the native valuation and offers only tabs with content",
  async () => {
    renderWorkbench();
    expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual([
      "Positions",
      "Risk",
      "Scenarios",
      "Margin",
      "Book",
    ]);
    const summary = await screen.findByRole(
      "region",
      { name: "Portfolio summary" },
      { timeout: budget(10000) },
    );
    expect(
      within(summary).getAllByTitle(
        fixture.report.valuation.total_base_currency.amount,
      ).length,
    ).toBeGreaterThan(0);
    // Rows follow the spec order the book returned.
    const positions = await screen.findByRole("region", { name: "Positions" });
    expect(
      rowsOf(within(positions).getAllByRole("table")[0]!).map(
        (row) => row.cells[0]!.textContent,
      ),
    ).toEqual(fixture.report.positionIds);
    // The Scenarios and Margin calls wait for their tabs.
    expect(harness.methods()).not.toContain("scenarioPnlBatch");
    expect(harness.methods()).not.toContain("portfolioMargin");
    expect(screen.queryAllByRole("alert")).toEqual([]);
  },
  budget(20000),
);
it(
  "shows the risk series, scenario P&L and margin as Rust returned them",
  async () => {
    renderWorkbench();
    await screen.findByRole(
      "region",
      { name: "Portfolio summary" },
      { timeout: budget(10000) },
    );
    await userEvent.click(tab("Risk"));
    expect(
      await screen.findByRole(
        "img",
        { name: "bucketed_dv01 USD-OIS portfolio totals by bucket" },
        { timeout: budget(10000) },
      ),
    ).toBeTruthy();
    await userEvent.click(tab("Scenarios"));
    const pnl = await screen.findByRole(
      "region",
      { name: "Portfolio scenario P&L" },
      { timeout: budget(10000) },
    );
    for (const item of fixture.batch.items)
      expect(
        within(pnl).getAllByTitle(item.pnl.total.amount).length,
      ).toBeGreaterThan(0);
    await userEvent.click(tab("Margin"));
    const margin = await screen.findByRole(
      "region",
      { name: "Portfolio margin" },
      { timeout: budget(10000) },
    );
    expect(
      within(margin).getAllByTitle(fixture.margin.total_margin.amount).length,
    ).toBeGreaterThan(0);
    expect(
      harness.methods().filter((call) => call === "scenarioPnlBatch"),
    ).toEqual(["scenarioPnlBatch"]);
    expect(
      harness.methods().filter((call) => call === "portfolioMargin"),
    ).toEqual(["portfolioMargin"]);
    expect(screen.queryAllByRole("alert")).toEqual([]);
  },
  budget(30000),
);
it("leaves out the Scenarios tab when the host supplies no scenarios", () => {
  renderWorkbench({ scenarioJsons: undefined });
  expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual([
    "Positions",
    "Risk",
    "Margin",
    "Book",
  ]);
});
it(
  "validates a replacement book natively before valuing it, and shows a rejection as returned",
  async () => {
    const user = userEvent.setup();
    renderWorkbench();
    await screen.findByRole(
      "region",
      { name: "Portfolio summary" },
      { timeout: budget(10000) },
    );
    await user.click(tab("Book"));
    const editor = screen.getByRole("textbox", { name: "PortfolioSpec JSON" });
    await user.clear(editor);
    await user.click(editor);
    await user.paste("{");
    await user.click(screen.getByRole("button", { name: "Validate & value" }));
    expect(
      (await screen.findByRole("alert", {}, { timeout: budget(5000) }))
        .textContent,
    ).not.toBe("");
    // The deposit-only book: the swaps are dropped from the host's spec.
    const spec = JSON.parse(specJson);
    const depositOnly = {
      ...spec,
      positions: spec.positions.filter(
        (position: { position_id: string }) => position.position_id === "DEP-1",
      ),
    };
    await user.clear(editor);
    await user.click(editor);
    await user.paste(JSON.stringify(depositOnly));
    await user.click(screen.getByRole("button", { name: "Validate & value" }));
    await waitFor(() => expect(screen.queryAllByRole("alert")).toEqual([]), {
      timeout: budget(5000),
    });
    await user.click(tab("Positions"));
    const positions = await screen.findByRole(
      "region",
      { name: "Positions" },
      { timeout: budget(10000) },
    );
    expect(
      rowsOf(within(positions).getAllByRole("table")[0]!).map(
        (row) => row.cells[0]!.textContent,
      ),
    ).toEqual(["DEP-1"]);
    expect(harness.methods()).toContain("validatePortfolio");
  },
  budget(30000),
);
it(
  "shows the native valuation error of a market the book cannot price on",
  async () => {
    renderWorkbench({ marketJson: "{}" });
    expect(
      (await screen.findByRole("alert", {}, { timeout: budget(10000) }))
        .textContent,
    ).not.toBe("");
  },
  budget(20000),
);
