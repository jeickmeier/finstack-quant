// @vitest-environment jsdom
import { createRequire } from "node:module";
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
import { FinstackQueryProvider } from "../registry/hooks/use-finstack/use-finstack";
import type { FinstackClient } from "../registry/hooks/use-finstack/client";
import { cashflowOptions } from "../registry/hooks/use-cashflows/use-cashflows";
import { CashflowViewer } from "../registry/components/cashflow-viewer/cashflow-viewer";
import {
  unwrap,
  type CashflowRequest,
} from "../registry/workers/finstack-contract";
import { startWorker } from "./worker/harness.mjs";
import fixture from "./cashflows/cases.json";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("../registry/hooks/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
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
}, 30000);
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
it("pins authoritative fixture sources and exact current native exports", () => {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
    ).toBe(source.sha256);
  for (const entry of fixture.cases) {
    const r = entry.request;
    expect(
      native.priceInstrument(
        r.instrumentJson,
        r.marketJson,
        r.asOf,
        r.model,
        r.metrics,
        r.pricingOptions,
        r.marketHistory,
      ).value,
    ).toEqual(entry.pricedValue);
    if (entry.cashflows !== null)
      expect(
        native.instrumentCashflowsJson(
          r.instrumentJson,
          r.marketJson,
          r.asOf,
          r.model,
        ),
      ).toBe(entry.cashflows);
    else
      expect(() =>
        native.instrumentCashflowsJson(
          r.instrumentJson,
          r.marketJson,
          r.asOf,
          r.model,
        ),
      ).toThrow(entry.error!);
  }
});
it.each(fixture.cases.filter((c) => c.cashflows !== null))(
  "tables and copies exact native $type cashflows at both densities",
  async (entry) => {
    const request = exportRequest(entry.request);
    const tree = (density: "compact" | "comfortable") => (
      <FinstackQueryProvider>
        <CashflowViewer request={request} density={density} />
      </FinstackQueryProvider>
    );
    const { rerender } = render(tree("compact"));
    const viewer = screen.getByRole("region", { name: "Cashflows" });
    await waitFor(() =>
      expect(
        within(viewer).getByRole("table", { name: "Cashflow schedule" }),
      ).toBeTruthy(),
    );
    const nativeRows = JSON.parse(entry.cashflows!).flows;
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
      ).toContain(`${JSON.parse(entry.cashflows!).currency} `);
    }
    fireEvent.click(within(viewer).getByText("Original JSON", { exact: true }));
    fireEvent.click(within(viewer).getByRole("button", { name: "Original" }));
    await waitFor(() =>
      expect(viewer.querySelector("pre")?.textContent).toBe(entry.cashflows),
    );
    for (const density of ["compact", "comfortable"] as const) {
      rerender(tree(density));
      expect(viewer.dataset.density).toBe(density);
      expect(viewer.querySelector("pre")!.textContent).toBe(entry.cashflows);
      fireEvent.click(within(viewer).getByRole("button", { name: "Copy" }));
      await waitFor(() =>
        expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(
          entry.cashflows,
        ),
      );
    }
    expect(
      call.mock.calls.filter(([method]) => method === "cashflows"),
    ).toEqual([["cashflows", request]]);
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
      <CashflowViewer request={exportRequest(entry.request)} />
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
  fireEvent.click(within(viewer).getByText("Original JSON", { exact: true }));
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
  await expect(query.fetchQuery(options)).resolves.toBe(
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

function suppliedExport(text: string) {
  call.mockImplementation((method, request) =>
    method === "cashflows" ? Promise.resolve(text) : invoke(method, request),
  );
  return render(
    <FinstackQueryProvider>
      <CashflowViewer request={exportRequest(fixture.cases[0].request)} />
    </FinstackQueryProvider>,
  );
}
it("preserves wide signed tokens, row currencies, zero and absent diagnostics without summing or rounding", async () => {
  const json = `{"instrument_id":"EXACT","currency":"USD","model":"discounting","as_of":"2025-01-01","total_pv":9007199254740993.123456789,"reconciles_with_base_value":false,"flows":[{"date":"2026-01-01","kind":"fixed","currency":"EUR","amount":-9007199254740993.123456789,"accrual_factor":0,"year_fraction":1.00000000000000001,"discount_factor":0,"discount_curve_id":"EUR-OIS","pv":0,"rate":0,"survival_probability":0}]}`;
  suppliedExport(json);
  const table = await screen.findByRole("table", { name: "Cashflow schedule" });
  expect(
    within(table).getByTitle("-9007199254740993.123456789").textContent,
  ).toBe("EUR -9,007,199,254,740,993.123456789");
  expect(
    within(table).getByTitle("9007199254740993.123456789").textContent,
  ).toBe("USD 9,007,199,254,740,993.123456789");
  expect(within(table).getByTitle("1.00000000000000001").textContent).toBe(
    "1.00000000000000001",
  );
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
  expect(totalRow.children[2].textContent).toBe(
    "USD 9,007,199,254,740,993.123456789",
  );
  expect(totalRow.children[3].textContent).toBe("");
});
it("omits absent optional columns and retains zero total for an empty native schedule", async () => {
  const envelope = JSON.parse(fixture.cases[0].cashflows!);
  envelope.flows = [];
  envelope.total_pv = 0;
  suppliedExport(JSON.stringify(envelope));
  await screen.findByText("No cashflows returned.");
  expect(screen.queryByRole("table")).toBeNull();
  expect(screen.getByText("Total PV · USD 0")).toBeTruthy();
});
it.each([
  "not JSON",
  '{"flows":[]}',
  fixture.cases[0].cashflows!.replace(
    '"amount":-1000000.0',
    '"amount":"-1000000.0"',
  ),
])(
  "rejects malformed/unsupported envelopes visibly and preserves the source",
  async (text) => {
    suppliedExport(text);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/^Cashflow table unavailable:/);
    expect(screen.queryByRole("table")).toBeNull();
    fireEvent.click(screen.getByText("Original JSON", { exact: true }));
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenLastCalledWith(text),
    );
  },
);
