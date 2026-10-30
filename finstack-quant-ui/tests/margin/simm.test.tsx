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
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ImResult, SimmSensitivitiesJson } from "finstack-quant-wasm";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  simmImOptions,
  useSimmIm,
  type SimmImRequest,
} from "@/hooks/margin/use-simm-im/use-simm-im";
import { SimmSensitivityTable } from "@/components/finstack/margin/components/simm-sensitivity-table/simm-sensitivity-table";
import { SimmReport } from "@/components/finstack/margin/components/simm-report/simm-report";
import { SimmCalculator } from "@/components/finstack/margin/components/simm-calculator/simm-calculator";
import { unwrap } from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "@/lib/finstack/fixtures/margin/simm.json";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
type Resources = Record<
  | "simmSensitivitiesConstructed"
  | "simmSensitivitiesFreed"
  | "simmCalculatorConstructed"
  | "simmCalculatorFreed",
  number
> & { simmCalculatorArgs: boolean[] };
let harness: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
/** Handles constructed and freed by one call. */
async function counted<T>(call: () => Promise<T>) {
  const before = await resources();
  const value = await call();
  const after = await resources();
  return {
    value,
    after,
    sensitivities: [
      after.simmSensitivitiesConstructed - before.simmSensitivitiesConstructed,
      after.simmSensitivitiesFreed - before.simmSensitivitiesFreed,
    ],
    calculator: [
      after.simmCalculatorConstructed - before.simmCalculatorConstructed,
      after.simmCalculatorFreed - before.simmCalculatorFreed,
    ],
  };
}
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
      if (method === "simmIm")
        return unwrap(await harness.proxy.simmIm(request as SimmImRequest));
      if (method === "validateSimmSensitivities")
        return unwrap(
          await harness.proxy.validateSimmSensitivities(request as string),
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
const example = fixture.example;
const anchor = fixture["ir-5y"];
const result = example.result as unknown as ImResult;
const usd = JSON.parse(anchor.request.sensitivitiesJson);
// Results are regenerated and compared by `mise run ui-fixtures --check`
// (tests/margin/generate.mjs).
it.each(["example", "ir-5y"] as const)(
  "returns the %s native result through the real worker, leaving options to Rust and freeing both handles",
  async (name) => {
    const { request, result } = fixture[name];
    const call = await counted(async () => await harness.proxy.simmIm(request));
    expect(unwrap(call.value)).toEqual(result);
    expect(call.sensitivities).toEqual([1, 1]);
    expect(call.calculator).toEqual([1, 1]);
    // Neither version nor MPOR was supplied to the calculator.
    expect(call.after.simmCalculatorArgs).toEqual([true, true]);
  },
  budget(15000),
);
it("pins the native USD 5Y anchor and the always-set approximation flag", () => {
  expect(anchor.result.amount).toEqual({ amount: "60000", currency: "USD" });
  expect(anchor.result.breakdown).toEqual({
    IR_Delta: { amount: "60000", currency: "USD" },
  });
  expect(example.result.approximation).toBe(true);
  expect(example.result.mpor_days).toBe(10);
});
it("passes an explicit MPOR to Rust", async () => {
  const value = unwrap(
    await harness.proxy.simmIm({ ...anchor.request, mporDays: 5 }),
  );
  expect(value.mpor_days).toBe(5);
  expect((await resources()).simmCalculatorArgs).toEqual([true, false]);
});
it("surfaces the native error for a non-USD set and frees both handles", async () => {
  const eur = await counted(
    async () =>
      await harness.proxy.simmIm({
        ...anchor.request,
        sensitivitiesJson: JSON.stringify({
          base_currency: "EUR",
          ir_delta: [["EUR", "5Y", 1000]],
        }),
      }),
  );
  expect(eur.value.ok).toBe(false);
  if (!eur.value.ok) {
    expect(eur.value.error.kind).toBe("validation");
    expect(eur.value.error.message).toContain("USD");
  }
  expect(eur.sensitivities).toEqual([1, 1]);
  expect(eur.calculator).toEqual([1, 1]);
  const version = await counted(
    async () =>
      await harness.proxy.simmIm({ ...anchor.request, version: "v9" }),
  );
  expect(version.value.ok).toBe(false);
  expect(version.sensitivities).toEqual([1, 1]);
  expect(version.calculator).toEqual([0, 0]);
});
it("validates natively to canonical text and returns native errors unchanged", async () => {
  const valid = await counted(
    async () =>
      await harness.proxy.validateSimmSensitivities(
        example.request.sensitivitiesJson,
      ),
  );
  expect(unwrap(valid.value)).toBe(example.canonical);
  expect(valid.sensitivities).toEqual([1, 1]);
  for (const [json, text] of [
    [JSON.stringify({ ...usd, foo: 1 }), "unknown field `foo`"],
    [
      JSON.stringify({ ...usd, ir_delta: [["USD", "7Q", 1]] }),
      "unknown tenor '7Q'",
    ],
  ] as const) {
    const failed = await harness.proxy.validateSimmSensitivities(json);
    expect(failed.ok).toBe(false);
    if (!failed.ok) expect(failed.error.message).toContain(text);
  }
});
it("keys the query on the worker session and the complete snapshotted request", () => {
  const options = simmImOptions(null, 3, anchor.request);
  expect(options.queryKey).toEqual(["finstack", 3, "simmIm", anchor.request]);
  expect(options.queryKey[3]).not.toBe(anchor.request);
  expect(options.queryKey[3]).not.toHaveProperty("version");
  expect(simmImOptions(null, 3, null).enabled).toBe(false);
});
it("lists supplied sensitivities by risk class without totals", () => {
  const sensitivities = JSON.parse(example.canonical) as SimmSensitivitiesJson;
  render(<SimmSensitivityTable sensitivities={sensitivities} />);
  const regions = screen
    .getAllByRole("region")
    .map((region) => region.getAttribute("aria-label"));
  expect(regions).toEqual([
    "SIMM sensitivities",
    "Interest rate",
    "Equity",
    "FX",
  ]);
  const ir = within(screen.getByRole("region", { name: "Interest rate" }));
  const rows = ir.getAllByRole("row").slice(1);
  // Input order and every supplied digit; no netting of the two tenors.
  expect(rows.map((row) => row.textContent)).toEqual([
    "USD10Y-8000",
    "USD5Y12500",
  ]);
  expect(screen.getByText(/base currency USD/)).toBeTruthy();
  expect(screen.queryByText("4500")).toBeNull();
});
it("presents the returned result, its indicative badge and the detail drill-down", async () => {
  const { rerender } = render(<SimmReport result={result} />);
  expect(screen.getByLabelText("Initial margin").textContent).toBe(
    "USD 13,958,205.61533609",
  );
  expect(screen.getByText("Indicative")).toBeTruthy();
  const breakdown = within(
    screen.getByRole("region", { name: "Breakdown" }),
  ).getAllByRole("row");
  expect(breakdown.slice(1).map((row) => row.textContent)).toEqual([
    "Equity_DeltaUSD 12,500,000",
    "FX_DeltaUSD 2,960,000",
    "IR_DeltaUSD 330,000",
  ]);
  const detail = screen.getByRole("region", { name: "Component detail" });
  expect(within(detail).getByText(/Bucket USD · K 330000/)).toBeTruthy();
  await userEvent.click(within(detail).getByRole("combobox"));
  await userEvent.click(
    await screen.findByRole("option", { name: "FX_Delta" }),
  );
  expect(within(detail).getByText(/Bucket fx · K 2960000/)).toBeTruthy();
  // The badge stays even when Rust does not flag an approximation.
  rerender(<SimmReport result={{ ...result, approximation: false }} />);
  expect(screen.getByText("Indicative")).toBeTruthy();
  expect(screen.getByText("Not flagged as an approximation")).toBeTruthy();
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<SimmReport result={null} />);
  expect(screen.getByRole("status").textContent).toBe("No SIMM result");
  rerender(<SimmReport result={null} loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Calculating SIMM initial margin…",
  );
  rerender(<SimmReport result={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
  rerender(<SimmSensitivityTable sensitivities={null} />);
  expect(screen.getByRole("status").textContent).toBe("No SIMM sensitivities");
});
function Connected({ request }: { request: SimmImRequest }) {
  const im = useSimmIm(request);
  return (
    <SimmReport
      result={im.data ?? null}
      loading={im.isFetching}
      error={im.error?.message}
    />
  );
}
it(
  "renders the worker result through the hook, then the native non-USD error",
  async () => {
    const { unmount } = render(
      <FinstackQueryProvider>
        <Connected request={anchor.request} />
      </FinstackQueryProvider>,
    );
    await waitFor(
      () =>
        expect(screen.getByLabelText("Initial margin").textContent).toBe(
          "USD 60,000",
        ),
      { timeout: budget(5000) },
    );
    unmount();
    render(
      <FinstackQueryProvider>
        <Connected
          request={{
            ...anchor.request,
            sensitivitiesJson: JSON.stringify({
              base_currency: "EUR",
              ir_delta: [["EUR", "5Y", 1000]],
            }),
          }}
        />
      </FinstackQueryProvider>,
    );
    await waitFor(
      () => expect(screen.getByRole("alert").textContent).toContain("USD"),
      { timeout: budget(5000) },
    );
  },
  budget(15000),
);
it("edits the schema example, shows the validated set and submits canonical text", async () => {
  const validate = vi.fn(async (json: string) => json);
  const onSubmit = vi.fn();
  render(
    <SimmCalculator
      asOf="2025-01-15"
      onAsOfChange={() => {}}
      validate={validate}
      onSubmit={onSubmit}
      result={result}
    />,
  );
  // The validated set appears once the caller's validation returns.
  await waitFor(
    () =>
      expect(
        screen.getByRole("region", { name: "Interest rate" }),
      ).toBeTruthy(),
    { timeout: budget(5000) },
  );
  expect(screen.getByLabelText("As of")).toHaveProperty("value", "2025-01-15");
  await userEvent.click(
    screen.getByRole("button", { name: "Calculate initial margin" }),
  );
  await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
  expect(JSON.parse(onSubmit.mock.calls[0]![0] as string)).toMatchObject({
    base_currency: "USD",
    equity_delta: [["AAPL", 250000]],
  });
  expect(screen.getByLabelText("Initial margin")).toBeTruthy();
});
