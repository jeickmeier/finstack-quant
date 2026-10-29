// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  beforeAll,
  beforeEach,
  afterAll,
  afterEach,
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
import { QueryClient } from "@tanstack/react-query";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  cashflowOptions,
  useCashflows,
} from "@/hooks/valuations/use-cashflows/use-cashflows";
import { CashflowViewer } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { unwrap, type CashflowRequest } from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import fixture from "./cashflows/cases.json";
const envelope = (value: unknown) => value as InstrumentCashflowWire;
import { budget } from "../shared/budget";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
let harness: Awaited<ReturnType<typeof startWorker>>;
let call: ReturnType<typeof vi.fn>;
const exportRequest = (request: CashflowRequest): CashflowRequest => ({
  instrumentJson: request.instrumentJson,
  marketJson: request.marketJson,
  asOf: request.asOf,
  model: request.model,
});
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
async function invoke(
  method: "initialize" | "cashflows",
  request: CashflowRequest,
) {
  return method === "initialize"
    ? unwrap(await harness.proxy.initialize())
    : unwrap(await harness.proxy.cashflows(request));
}
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(invoke);
  mock.createClient.mockReturnValue({ call, close() {} });
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
// Prices and cashflow exports are regenerated and compared by
// `mise run ui-fixtures --check` (tests/valuations/cashflows/generate.mjs).
it("pins the JSON inputs the cashflow fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/valuations/cashflows/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it.each(fixture.cases.filter((c) => c.cashflows !== null))(
  "tables and copies the native $type envelope at both densities",
  async (entry) => {
    const cashflows = envelope(entry.cashflows);
    const text = serializeHost(cashflows);
    const tree = (density: "compact" | "comfortable") => (
      <CashflowViewer cashflows={cashflows} density={density} />
    );
    const { rerender } = render(tree("compact"));
    const viewer = screen.getByRole("region", { name: "Cashflows" });
    await waitFor(() =>
      expect(
        within(viewer).getByRole("table", { name: "Cashflow schedule" }),
      ).toBeTruthy(),
    );
    const nativeRows = cashflows.flows;
    expect(viewer.querySelectorAll("tbody tr")).toHaveLength(nativeRows.length);
    expect(viewer.querySelector("details")!.open).toBe(false);
    expect(viewer.querySelector("details")!.className).toContain(
      "print:hidden",
    );
    const rows = viewer.querySelectorAll("tbody tr");
    for (const [i, row] of nativeRows.entries()) {
      expect(rows[i].children[0].firstElementChild?.textContent).toBe(row.date);
      expect(rows[i].children[1].textContent).toBe(row.kind);
      expect(rows[i].children[2].textContent).toContain(`${row.currency} `);
      expect(
        rows[i].children[rows[i].children.length - 1].textContent,
      ).toContain(`${cashflows.currency} `);
    }
    fireEvent.click(within(viewer).getByText("Envelope JSON", { exact: true }));
    fireEvent.click(within(viewer).getByRole("button", { name: "Original" }));
    await waitFor(() =>
      expect(viewer.querySelector("pre")?.textContent).toBe(text),
    );
    for (const density of ["compact", "comfortable"] as const) {
      rerender(tree(density));
      expect(viewer.dataset.density).toBe(density);
      expect(viewer.querySelector("pre")!.textContent).toBe(text);
      fireEvent.click(within(viewer).getByRole("button", { name: "Copy" }));
      await waitFor(() =>
        expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(text),
      );
    }
    expect(
      call.mock.calls.filter(([method]) => method === "cashflows"),
    ).toEqual([]);
  },
);
it("shows loading, then the actual unsupported error without changing caller valuation or model", async () => {
  const entry = fixture.cases.find((c) => c.error)!;

  let finish!: () => void;
  call.mockImplementation(async (method, request) => {
    if (method === "cashflows")
      await new Promise<void>((resolve) => {
        finish = resolve;
      });
    return invoke(method, request);
  });
  const print = vi.spyOn(window, "print").mockImplementation(() => {});
  render(
    <FinstackQueryProvider>
      <output aria-label="Caller valuation">{entry.pricedValue.amount}</output>
      <output aria-label="Selected model">{entry.request.model}</output>
      <ConnectedCashflows request={exportRequest(entry.request)} />
    </FinstackQueryProvider>,
  );
  const viewer = screen.getByRole("region", { name: "Cashflows" });
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  expect(viewer.getAttribute("aria-busy")).toBe("true");
  finish();
  await waitFor(() =>
    expect(within(viewer).getByRole("alert").textContent).toBe(
      `Cashflows unavailable: ${entry.error}`,
    ),
  );
  expect(viewer.getAttribute("aria-busy")).toBe("false");
  expect(screen.getByLabelText("Caller valuation").textContent).toBe(
    entry.pricedValue.amount,
  );
  expect(screen.getByLabelText("Selected model").textContent).toBe(
    entry.request.model,
  );
  fireEvent.click(within(viewer).getByText("Envelope JSON", { exact: true }));
  expect(
    (within(viewer).getByRole("button", { name: "Copy" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  fireEvent.click(within(viewer).getByRole("button", { name: "Print" }));
  expect(print).toHaveBeenCalledTimes(1);
  print.mockRestore();
});
it("snapshots every export input and separates worker sessions in the query key", async () => {
  const client = { call, close() {} } as FinstackClient;
  const query = new QueryClient();
  const request = { ...exportRequest(fixture.cases[0].request) };
  const options = cashflowOptions(client, 10, request);
  request.model = "black76";
  expect(options.queryKey.at(-1)).toMatchObject({ model: "discounting" });
  await expect(query.fetchQuery(options)).resolves.toEqual(
    fixture.cases[0].cashflows,
  );
  const original = exportRequest(fixture.cases[0].request);
  for (const field of [
    "instrumentJson",
    "marketJson",
    "asOf",
    "model",
  ] as const)
    expect(
      cashflowOptions(client, 10, {
        ...original,
        [field]: original[field] + " ",
      }).queryKey,
    ).not.toEqual(options.queryKey);
  expect(cashflowOptions(client, 11, original).queryKey).not.toEqual(
    options.queryKey,
  );
  expect(original.model).toBe("discounting");
  query.clear();
});

function suppliedExport(cashflows: unknown) {
  return render(<CashflowViewer cashflows={envelope(cashflows)} />);
}
function ConnectedCashflows({ request }: { request: CashflowRequest }) {
  const query = useCashflows(request);
  return (
    <CashflowViewer
      cashflows={query.data}
      loading={query.isFetching || query.workerStatus === "starting"}
      error={query.error?.message}
    />
  );
}
it("presents returned numbers, row currencies, zero and absent diagnostics without summing or rounding", async () => {
  const cashflows = {
    instrument_id: "EXACT",
    discount_curve_id: "USD-OIS",
    currency: "USD",
    model: "discounting",
    as_of: "2025-01-01",
    total_pv: 1234567.125,
    reconciles_with_base_value: false,
    flows: [
      {
        date: "2026-01-01",
        kind: "fixed",
        currency: "EUR",
        amount: -1e-7,
        accrual_factor: 0,
        year_fraction: 1.0000000000000002,
        discount_factor: 0,
        discount_curve_id: "EUR-OIS",
        native_pv: 0,
        pv: 0,
        rate: null,
        survival_probability: 0,
      },
    ],
  };
  suppliedExport(cashflows);
  const table = await screen.findByRole("table", { name: "Cashflow schedule" });
  expect(within(table).getByTitle("-1e-7").textContent).toBe("EUR -0.0000001");
  expect(within(table).getByTitle("1234567.125").textContent).toBe(
    "USD 1,234,567.125",
  );
  expect(within(table).getByTitle("1.0000000000000002").textContent).toBe(
    "1.0000000000000002",
  );
  // A null optional is absent: no Rate column when no row carries one.
  expect(
    within(table).queryByRole("columnheader", { name: "Rate" }),
  ).toBeNull();
  expect(
    within(table).getByRole("columnheader", { name: "PV · USD" }),
  ).toBeTruthy();
  expect(
    within(table)
      .getAllByText("Survival")
      .every((node) => node.nextElementSibling?.textContent === "0"),
  ).toBe(true);
  expect(screen.getByText("Not reconciled to base value")).toBeTruthy();
  const totalRow = table.querySelector("tfoot tr")!;
  expect(totalRow.children).toHaveLength(4);
  expect(totalRow.children[2].textContent).toBe("USD 1,234,567.125");
  expect(totalRow.children[3].textContent).toBe("");
  fireEvent.click(screen.getByText("Envelope JSON", { exact: true }));
  fireEvent.click(screen.getByRole("button", { name: "Copy" }));
  await waitFor(() =>
    expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
      serializeHost(cashflows),
    ),
  );
});
it("keeps the schedule a named keyboard stop inside its scroll container", async () => {
  suppliedExport(fixture.cases[0].cashflows);
  // Wide schedules overflow narrow viewports; focusing the table lets arrow
  // keys scroll the stock table container around it.
  const table = await screen.findByRole("table", { name: "Cashflow schedule" });
  expect(table.tabIndex).toBe(0);
  expect(table.parentElement!.dataset.slot).toBe("table-container");
});
it("omits absent optional columns and retains zero total for an empty native schedule", async () => {
  suppliedExport({ ...fixture.cases[0].cashflows, flows: [], total_pv: 0 });
  await screen.findByText("No cashflows returned.");
  expect(screen.queryByRole("table")).toBeNull();
  expect(screen.getByText("Total PV · USD 0")).toBeTruthy();
});
const { discount_curve_id: _omitted, ...withoutCurve } =
  fixture.cases[0].cashflows!;
it.each([
  withoutCurve,
  { flows: [] },
  {
    ...fixture.cases[0].cashflows,
    flows: fixture.cases[0].cashflows!.flows.map((row, index) =>
      index === 0 ? { ...row, amount: String(row.amount) } : row,
    ),
  },
])(
  "rejects malformed/unsupported envelopes visibly and still serializes the supplied object",
  async (cashflows) => {
    suppliedExport(cashflows);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/^Cashflow table unavailable:/);
    expect(screen.queryByRole("table")).toBeNull();
    fireEvent.click(screen.getByText("Envelope JSON", { exact: true }));
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
        serializeHost(cashflows),
      ),
    );
  },
);
