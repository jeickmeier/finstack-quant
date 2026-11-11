// @vitest-environment jsdom
import { resolve } from "node:path";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
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
import {
  cashflowLadderOptions,
  useCashflowLadder,
} from "@/hooks/valuations/use-cashflow-ladder/use-cashflow-ladder";
import { CashflowViewer } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import type { InstrumentCashflowWire } from "@/lib/finstack/generated/types/instrument_cashflow";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  unwrap,
  type CashflowEnvelope,
  type CashflowRequest,
} from "@/workers/finstack-contract";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import fixture from "./cashflows/cases.json";
const envelope = (value: unknown) => value as InstrumentCashflowWire;
import { budget } from "../shared/budget";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
const exportRequest = (request: CashflowRequest): CashflowRequest => ({
  instrumentJson: request.instrumentJson,
  marketJson: request.marketJson,
  asOf: request.asOf,
  model: request.model,
});
const harness = useWorkerHarness(["cashflows", "cashflowLadder"]);
const { call } = harness;
beforeEach(() => {
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
        [...rows[i].children].some(
          (cell) =>
            cell.getAttribute("title") === String(row.pv) &&
            cell.textContent?.includes(`${cashflows.currency} `),
        ),
      ).toBe(true);
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
  const route = call.getMockImplementation()!;
  call.mockImplementation(async (method, request) => {
    if (method === "cashflows")
      await new Promise<void>((resolve) => {
        finish = resolve;
      });
    return route(method, request);
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
const nativeFlows = (type: string) =>
  envelope(fixture.cases.find((c) => c.type === type)!.cashflows).flows;
const balanceTable = async (type: string) => {
  suppliedExport(fixture.cases.find((c) => c.type === type)!.cashflows);
  return screen.findByRole("table", { name: "Cashflow schedule" });
};
it("shows how a PIK loan accretes in the balances Rust supplies, each once", async () => {
  const flows = nativeFlows("term_loan_pik");
  const table = await balanceTable("term_loan_pik");
  const headers = within(table)
    .getAllByRole("columnheader")
    .map((header) => header.textContent);
  expect(headers).toContain("Principal before");
  expect(headers).toContain("Principal after");
  expect(within(table).queryByText("Beginning balance")).toBeNull();
  expect(within(table).queryByText("Ending balance")).toBeNull();
  const before = headers.indexOf("Principal before");
  const after = headers.indexOf("Principal after");
  const rows = table.querySelectorAll("tbody tr");
  // The issue funding row starts from zero, then every PIK coupon moves the balance.
  expect(flows[0].kind).toBe("notional");
  expect(flows[0].beginning_balance).toBe(0);
  expect(rows[0].children[before].textContent).toBe("USD 0");
  const pik = flows.findIndex((row) => row.kind === "pik");
  expect(flows[pik].ending_balance).toBeGreaterThan(
    flows[pik].beginning_balance!,
  );
  for (const [i, row] of flows.entries()) {
    expect(rows[i].children[before].getAttribute("title")).toBe(
      String(row.beginning_balance),
    );
    expect(rows[i].children[after].getAttribute("title")).toBe(
      String(row.ending_balance),
    );
    expect(rows[i].children[after].textContent).toContain(`${row.currency} `);
  }
  const footer = table.querySelector("tfoot tr")!;
  expect(footer.children[0].getAttribute("colspan")).toBe(
    String(headers.length - 2),
  );
  expect(
    screen.getByText(/an issue funding row starts from zero/),
  ).toBeTruthy();
});
it("shows the shared per-period balances of an agency MBS pool as supplied", async () => {
  const flows = nativeFlows("agency_mbs_passthrough");
  const table = await balanceTable("agency_mbs_passthrough");
  const headers = within(table)
    .getAllByRole("columnheader")
    .map((header) => header.textContent);
  const before = headers.indexOf("Principal before");
  const period = flows.filter((row) => row.date === flows[0].date);
  expect(period.length).toBeGreaterThan(1);
  expect(new Set(period.map((row) => row.beginning_balance)).size).toBe(1);
  const rows = table.querySelectorAll("tbody tr");
  for (const [i, row] of period.entries())
    expect(rows[i].children[before].getAttribute("title")).toBe(
      String(row.beginning_balance),
    );
});
it("hides the balance columns only when Rust supplied none", async () => {
  // A swap's legs carry a reference notional, not an outstanding balance.
  expect(
    nativeFlows("xccy_swap").every((row) => row.beginning_balance == null),
  ).toBe(true);
  const table = await balanceTable("xccy_swap");
  expect(
    within(table).queryByRole("columnheader", { name: "Principal before" }),
  ).toBeNull();
  expect(
    within(table).queryByRole("columnheader", { name: "Principal after" }),
  ).toBeNull();
  expect(screen.queryByText(/an issue funding row/)).toBeNull();
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

const withFlows = fixture.cases.filter((c) => c.cashflows !== null);
it.each(withFlows)(
  "returns the native $type ladder per row currency in first-appearance order",
  async (entry) => {
    const cashflows = entry.cashflows as unknown as CashflowEnvelope;
    const ladder = unwrap(await harness.proxy.cashflowLadder(cashflows));
    expect(ladder).toEqual(entry.ladder);
    expect(ladder.map((currency) => currency.currency)).toEqual([
      ...new Set(cashflows.flows.map((flow) => flow.currency)),
    ]);
  },
);
it("ladders the two-currency cross-currency swap per currency, never across them", () => {
  const entry = fixture.cases.find((c) => c.type === "xccy_swap")!;
  expect(
    entry.ladder!.map(({ currency, rows }) => [currency, rows.length]),
  ).toEqual([
    ["EUR", 6],
    ["USD", 6],
  ]);
});
it("returns the native validation error for a flow kind Rust does not know", async () => {
  const cashflows = withFlows[0].cashflows as unknown as CashflowEnvelope;
  const result = await harness.proxy.cashflowLadder({
    ...cashflows,
    flows: cashflows.flows.map((flow, index) =>
      index === 0 ? { ...flow, kind: "not_a_kind" as never } : flow,
    ),
  });
  expect(result.ok).toBe(false);
  if (result.ok) return;
  expect(result.error.kind).toBe("validation");
  expect(result.error.message).toContain("not_a_kind");
});
it("keys the ladder on the worker session and the complete envelope", () => {
  const client = { call, close() {} } as FinstackClient;
  const cashflows = withFlows[0].cashflows as unknown as CashflowEnvelope;
  const options = cashflowLadderOptions(client, 3, cashflows);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "cashflowLadder",
    cashflows,
  ]);
  expect(cashflowLadderOptions(client, 4, cashflows).queryKey).not.toEqual(
    options.queryKey,
  );
  expect(cashflowLadderOptions(client, 3, null).enabled).toBe(false);
});
it("offers the annual ladder only when one is supplied and tables each currency in native PV", async () => {
  const entry = fixture.cases.find((c) => c.type === "xccy_swap")!;
  const cashflows = envelope(entry.cashflows);
  const { rerender } = render(<CashflowViewer cashflows={cashflows} />);
  await screen.findByRole("table", { name: "Cashflow schedule" });
  expect(screen.queryByRole("radio", { name: "Annual ladder" })).toBeNull();
  rerender(
    <CashflowViewer cashflows={cashflows} ladder={entry.ladder as never} />,
  );
  expect(
    (
      screen.getByRole("radio", { name: "Schedule" }) as HTMLElement
    ).getAttribute("aria-checked"),
  ).toBe("true");
  fireEvent.click(screen.getByRole("radio", { name: "Annual ladder" }));
  const group = await screen.findByRole("group", { name: "Annual ladder" });
  expect(screen.queryByRole("table", { name: "Cashflow schedule" })).toBeNull();
  for (const { currency, rows } of entry.ladder!) {
    const table = within(group).getByRole("table", {
      name: `Annual ladder · ${currency}`,
    });
    expect(within(table).getByRole("columnheader", { name: "Native PV" }));
    const body = table.querySelectorAll("tbody tr");
    expect(body).toHaveLength(rows.length);
    for (const [i, row] of rows.entries()) {
      expect(body[i].children[0].textContent).toBe(String(row.year));
      expect(
        body[i].children[3].querySelector("[title]")?.getAttribute("title"),
      ).toBe(String(row.pv));
      expect(body[i].children[3].textContent).toContain(`${currency} `);
    }
  }
});
it("shows the pending and native-error ladder states", async () => {
  const cashflows = envelope(withFlows[0].cashflows);
  const { rerender } = render(
    <CashflowViewer cashflows={cashflows} ladder={null} />,
  );
  fireEvent.click(await screen.findByRole("radio", { name: "Annual ladder" }));
  expect(screen.getByText("Loading annual ladder…")).toBeTruthy();
  rerender(
    <CashflowViewer
      cashflows={cashflows}
      ladder={null}
      ladderError="bad kind"
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(
    "Annual ladder unavailable: bad kind",
  );
});
function ConnectedLadder({ request }: { request: CashflowRequest }) {
  const cashflows = useCashflows(request);
  const ladder = useCashflowLadder(cashflows.data);
  return (
    <CashflowViewer
      cashflows={cashflows.data}
      ladder={ladder.data ?? null}
      ladderError={ladder.error?.message}
    />
  );
}
it("chains the worker ladder onto the worker envelope through the hooks", async () => {
  const entry = fixture.cases.find((c) => c.type === "xccy_swap")!;
  render(
    <FinstackQueryProvider>
      <ConnectedLadder request={exportRequest(entry.request)} />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("radio", { name: "Annual ladder" }));
  await screen.findByRole("table", { name: "Annual ladder · EUR" });
  expect(
    screen.getByRole("table", { name: "Annual ladder · USD" }),
  ).toBeTruthy();
  expect(
    call.mock.calls.filter(([method]) => method === "cashflowLadder"),
  ).toEqual([["cashflowLadder", entry.cashflows]]);
});
