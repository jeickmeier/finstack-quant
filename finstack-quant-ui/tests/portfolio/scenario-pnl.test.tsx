// @vitest-environment jsdom
import { readFileSync } from "node:fs";
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
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { ScenarioPnlBatchItem } from "finstack-quant-wasm";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  scenarioPnlBatchOptions,
  useScenarioPnlBatch,
  type ScenarioPnlBatchRequest,
} from "@/hooks/portfolio/use-scenario-pnl-batch/use-scenario-pnl-batch";
import { applyScenarioAndRevalueOptions } from "@/hooks/portfolio/use-apply-scenario-and-revalue/use-apply-scenario-and-revalue";
import {
  ScenarioPnlTable,
  type ScenarioPnlTableData,
} from "@/components/finstack/portfolio/components/scenario-pnl-table/scenario-pnl-table";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./scenario-pnl.json";
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
const { inputs, expected } = JSON.parse(
  readFileSync(
    resolve(
      dirname(fileURLToPath(import.meta.url)),
      "../../../finstack-quant/portfolio/tests/fixtures/host_parity.json",
    ),
    "utf8",
  ),
);
let harness: Awaited<ReturnType<typeof startWorker>>;
const proxy = () => harness.proxy as unknown as WorkerApi;
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
const batchRequest: ScenarioPnlBatchRequest = {
  specJson: JSON.stringify(inputs.portfolio_spec),
  marketJson: JSON.stringify(inputs.market_t0),
  scenarioJsons: fixture.request.scenarioJsons,
};
const batch = fixture.batch as unknown as ScenarioPnlTableData;
const rowsOf = (table: HTMLElement) =>
  within(table).getAllByRole("row") as HTMLTableRowElement[];
// Fixtures are regenerated and compared by `mise run ui-fixtures --check`
// (tests/portfolio/generate.mjs).
it(
  "values the parity scenarios through the real worker as the generator did and as the parity tests pin",
  async () => {
    const result = unwrap(await proxy().scenarioPnlBatch(batchRequest));
    expect(serializeHost(result)).toBe(serializeHost(fixture.batch));
    expect(result.items.map((item) => item.scenario_id)).toEqual([
      "up_10bp",
      "down_15bp",
    ]);
    expect(result.items.map((item) => item.pnl)).toEqual(
      expected.scenario_pnl_batch.map((item: ScenarioPnlBatchItem) => item.pnl),
    );
    const revalue = unwrap(
      await proxy().applyScenarioAndRevalue({
        specJson: batchRequest.specJson,
        marketJson: batchRequest.marketJson,
        scenarioJson: fixture.revalue.scenarioJson,
      }),
    );
    expect(serializeHost(revalue)).toBe(
      serializeHost({
        positionIds: fixture.revalue.positionIds,
        entityIds: fixture.revalue.entityIds,
        view: fixture.revalue.view,
      }),
    );
  },
  budget(15000),
);
it("returns an empty batch without valuing and the earliest failing scenario's native error", async () => {
  expect(
    unwrap(
      await proxy().scenarioPnlBatch({ ...batchRequest, scenarioJsons: [] }),
    ).items,
  ).toEqual([]);
  const failed = await proxy().scenarioPnlBatch({
    ...batchRequest,
    scenarioJsons: [
      batchRequest.scenarioJsons[0]!,
      JSON.stringify({ id: "", operations: [] }),
    ],
  });
  expect(failed.ok).toBe(false);
  if (!failed.ok) expect(failed.error.kind).toBe("validation");
  const missing = await proxy().applyScenarioAndRevalue({
    specJson: batchRequest.specJson,
    marketJson: batchRequest.marketJson,
    scenarioJson: JSON.stringify({
      id: "missing",
      operations: [
        {
          kind: "curve_parallel_bp",
          curve_kind: "discount",
          curve_id: "NO-SUCH-CURVE",
          bp: 10,
        },
      ],
    }),
  });
  expect(missing.ok).toBe(false);
});
it("keys both queries on the worker session and the complete snapshotted request", () => {
  const options = scenarioPnlBatchOptions(null, 2, batchRequest);
  expect(options.queryKey).toEqual([
    "finstack",
    2,
    "scenarioPnlBatch",
    batchRequest,
  ]);
  expect(options.queryKey[3]).not.toBe(batchRequest);
  expect(scenarioPnlBatchOptions(null, 2, null).enabled).toBe(false);
  const revalue = {
    specJson: batchRequest.specJson,
    marketJson: batchRequest.marketJson,
    scenarioJson: fixture.revalue.scenarioJson,
  };
  expect(applyScenarioAndRevalueOptions(null, 3, revalue).queryKey).toEqual([
    "finstack",
    3,
    "applyScenarioAndRevalue",
    revalue,
  ]);
  expect(applyScenarioAndRevalueOptions(null, 3, null).enabled).toBe(false);
});
it("shows positions by scenarios as returned with a total row from each returned total", () => {
  render(<ScenarioPnlTable report={batch} density="comfortable" />);
  const table = screen.getByRole("table", {
    name: "P&L by position and scenario",
  });
  const rows = rowsOf(table);
  expect([...rows[0]!.cells].map((cell) => cell.textContent)).toEqual([
    "Position",
    "up_10bp",
    "down_15bp",
  ]);
  const title = (cell: HTMLTableCellElement) =>
    cell.querySelector("[title]")!.getAttribute("title");
  const body = rows.slice(1);
  expect(body.map((row) => row.cells[0]!.textContent)).toEqual([
    "POS-0",
    "POS-1",
    "Total",
  ]);
  batch.items.forEach((item, column) => {
    expect(title(body[0]!.cells[column + 1]!)).toBe(
      item.pnl.by_position["POS-0"]!.amount,
    );
    // The total is Rust's, never a sum of the rows above it.
    expect(title(body[2]!.cells[column + 1]!)).toBe(item.pnl.total.amount);
  });
  expect(screen.getByRole("img").getAttribute("aria-label")).toBe(
    "Portfolio P&L by scenario",
  );
});
it("keeps the book's position order for integer-like ids and lists a scenario-added position", () => {
  const items = batch.items.map((item) => ({
    ...item,
    pnl: {
      ...item.pnl,
      // JavaScript enumerates "2" before "10"; the rows must not.
      by_position: {
        "10": item.pnl.by_position["POS-0"]!,
        "2": item.pnl.by_position["POS-1"]!,
        NEW: item.pnl.total,
      },
    },
  }));
  render(<ScenarioPnlTable report={{ positionIds: ["10", "2"], items }} />);
  const body = rowsOf(
    screen.getByRole("table", { name: "P&L by position and scenario" }),
  ).slice(1);
  expect(body.map((row) => row.cells[0]!.textContent)).toEqual([
    "10",
    "2",
    "NEW",
    "Total",
  ]);
  expect(screen.getByText(/Positions not in the book \(NEW\)/)).toBeTruthy();
});
it("renders each scenario report only once its disclosure opens", () => {
  render(<ScenarioPnlTable report={batch} />);
  expect(
    screen.queryByRole("region", { name: "Application report" }),
  ).toBeNull();
  const disclosure = screen
    .getByText(/up_10bp/, { selector: "span" })
    .closest("details")!;
  disclosure.open = true;
  fireEvent(disclosure, new Event("toggle"));
  const json = screen.getByRole("region", { name: "Application report" });
  expect(JSON.parse(json.querySelector("code")!.textContent!)).toEqual(
    batch.items[0]!.report,
  );
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<ScenarioPnlTable report={null} />);
  expect(screen.getByRole("status").textContent).toBe("No scenario P&L");
  rerender(<ScenarioPnlTable report={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Valuing scenarios…");
  rerender(<ScenarioPnlTable report={batch} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
function Connected({ request }: { request: ScenarioPnlBatchRequest }) {
  const result = useScenarioPnlBatch(request);
  return (
    <ScenarioPnlTable
      report={result.data}
      loading={result.isFetching}
      error={result.error?.message}
    />
  );
}
it("connects the hook to the table through the provider", async () => {
  render(
    <FinstackQueryProvider>
      <Connected request={batchRequest} />
    </FinstackQueryProvider>,
  );
  const table = await screen.findByRole(
    "table",
    { name: "P&L by position and scenario" },
    { timeout: budget(5000) },
  );
  await waitFor(() =>
    expect(rowsOf(table).at(-1)!.cells[1]!.textContent).toContain("-1,487"),
  );
});
