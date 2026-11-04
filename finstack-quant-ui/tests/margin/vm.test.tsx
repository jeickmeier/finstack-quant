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
import type { CsaSpec, MarginCall, VmResult } from "finstack-quant-wasm";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import { useCsaPreset } from "@/hooks/margin/use-csa-preset/use-csa-preset";
import { useCsaValidator } from "@/hooks/margin/use-csa-validator/use-csa-validator";
import { useVm, vmOptions, type VmRequest } from "@/hooks/margin/use-vm/use-vm";
import {
  marginCallsOptions,
  useMarginCalls,
} from "@/hooks/margin/use-margin-calls/use-margin-calls";
import {
  marginCallDatesOptions,
  useMarginCallDates,
} from "@/hooks/margin/use-margin-call-dates/use-margin-call-dates";
import { CsaEditor } from "@/components/finstack/margin/components/csa-editor/csa-editor";
import { MarginCallTable } from "@/components/finstack/margin/components/margin-call-table/margin-call-table";
import {
  unwrap,
  type CsaPresetRequest,
  type MarginCallsRequest,
} from "@/workers/finstack-contract";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import fixture from "@/lib/finstack/fixtures/margin/vm.json";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
type Resources = Record<
  "vmCalculatorConstructed" | "vmCalculatorFreed",
  number
>;
const harness = useWorkerHarness([
  "csaPreset",
  "validateCsa",
  "vm",
  "marginCalls",
  "marginCallDates",
]);
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
/** VmCalculator handles constructed and freed by one call. */
async function counted<T>(call: () => Promise<T>) {
  const before = await resources();
  const value = await call();
  const after = await resources();
  return {
    value,
    calculator: [
      after.vmCalculatorConstructed - before.vmCalculatorConstructed,
      after.vmCalculatorFreed - before.vmCalculatorFreed,
    ],
  };
}
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const csa = JSON.parse(fixture.csaJson) as CsaSpec;
const collect = fixture.vm.collect;
const report = (name: keyof typeof fixture.vm) => ({
  ...fixture.vm[name].report,
  result: fixture.vm[name].report.result as unknown as VmResult,
});
const calls = fixture.calls.result as unknown as MarginCall[];
// JSON imports widen the [date, exposure] tuples to arrays.
const callsRequest = fixture.calls.request as unknown as MarginCallsRequest;
// Results are regenerated and compared by `mise run ui-fixtures --check`
// (tests/margin/generate.mjs).
it.each(["collect", "post", "below-mta"] as const)(
  "returns the %s native VmResult with Rust's call decision and net margin, freeing the calculator",
  async (name) => {
    const { request, report } = fixture.vm[name];
    const call = await counted(async () => await harness.proxy.vm(request));
    expect(unwrap(call.value)).toEqual(report);
    expect(call.calculator).toEqual([1, 1]);
  },
  budget(15000),
);
it("pins the native decisions: collect and post call, a sub-MTA exposure does not", () => {
  expect(fixture.vm.collect.report.requiresCall).toBe(true);
  expect(fixture.vm.collect.report.netMargin).toBe(-1_000_000);
  expect(fixture.vm.post.report.netMargin).toBe(750_000);
  expect(fixture.vm["below-mta"].report.requiresCall).toBe(false);
  expect(fixture.vm["below-mta"].report.result.unrounded_call).toEqual({
    amount: "100000",
    currency: "USD",
  });
});
it("returns the Rust presets, the canonical CSA, the call schedule and the call dates", async () => {
  expect(unwrap(await harness.proxy.csaPreset({ kind: "usd" }))).toEqual(
    fixture.presets.usd,
  );
  expect(unwrap(await harness.proxy.csaPreset({ kind: "eur" }))).toEqual(
    fixture.presets.eur,
  );
  expect(
    unwrap(
      await harness.proxy.csaPreset({
        kind: "regulatory",
        currency: "GBP",
        id: "CSA-GBP",
        collateralCurve: "GBP-SONIA",
      }),
    ),
  ).toEqual(fixture.presets.gbp);
  const valid = await counted(
    async () => await harness.proxy.validateCsa(fixture.csaJson),
  );
  expect(unwrap(valid.value)).toBe(fixture.canonical);
  expect(valid.calculator).toEqual([1, 1]);
  const schedule = await counted(
    async () => await harness.proxy.marginCalls(callsRequest),
  );
  expect(unwrap(schedule.value)).toEqual(fixture.calls.result);
  expect(schedule.calculator).toEqual([1, 1]);
  const dates = await counted(
    async () => await harness.proxy.marginCallDates(fixture.callDates.request),
  );
  expect(unwrap(dates.value)).toEqual(fixture.callDates.result);
  expect(dates.calculator).toEqual([1, 1]);
});
it("returns native errors unchanged and frees the calculator", async () => {
  const preset = await harness.proxy.csaPreset({
    kind: "regulatory",
    currency: "XYZ",
    id: "CSA",
    collateralCurve: "OIS",
  });
  expect(preset.ok).toBe(false);
  if (!preset.ok) expect(preset.error.message).toContain("XYZ");
  const calendar = await counted(
    async () =>
      await harness.proxy.validateCsa(
        JSON.stringify({ ...csa, calendar_id: "nope" }),
      ),
  );
  expect(calendar.value.ok).toBe(false);
  if (!calendar.value.ok)
    expect(calendar.value.error.message).toContain("calendar 'nope'");
  // csaSpecValidate rejects before any calculator is built.
  expect(calendar.calculator).toEqual([0, 0]);
  const unknown = await harness.proxy.validateCsa(
    JSON.stringify({ ...csa, extra: 1 }),
  );
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) expect(unknown.error.message).toContain("unknown field");
  // A cleared exposure is NaN, never 0: Rust rejects it.
  const cleared = await counted(
    async () => await harness.proxy.vm({ ...collect.request, exposure: NaN }),
  );
  expect(cleared.value.ok).toBe(false);
  if (!cleared.value.ok)
    expect(cleared.value.error.message).toContain("Non-finite");
  expect(cleared.calculator).toEqual([1, 1]);
});
it("keys every query on the worker session and the complete snapshotted request", () => {
  const options = vmOptions(null, 3, collect.request);
  expect(options.queryKey).toEqual(["finstack", 3, "vm", collect.request]);
  expect(options.queryKey[3]).not.toBe(collect.request);
  expect(vmOptions(null, 3, null).enabled).toBe(false);
  expect(marginCallsOptions(null, 2, callsRequest).queryKey).toEqual([
    "finstack",
    2,
    "marginCalls",
    callsRequest,
  ]);
  expect(
    marginCallDatesOptions(null, 2, fixture.callDates.request).queryKey,
  ).toEqual(["finstack", 2, "marginCallDates", fixture.callDates.request]);
});
it("presents the returned result, Rust's decision and net margin, the schedule and the call dates", () => {
  render(
    <MarginCallTable
      vm={report("collect")}
      calls={calls}
      callDates={fixture.callDates.result}
    />,
  );
  const vm = within(screen.getByRole("region", { name: "Variation margin" }));
  expect(vm.getByText("Margin call")).toBeTruthy();
  expect(vm.getByLabelText("Net margin").textContent).toBe(
    "Net margin USD -1000000",
  );
  expect(vm.getByText("As of 2024-06-17 · settles 2024-06-18")).toBeTruthy();
  const rows = vm.getAllByRole("row").slice(1);
  expect(rows).toHaveLength(10);
  expect(rows.at(-1)!.textContent).toContain("USD 1,000,000");
  const schedule = within(screen.getByRole("region", { name: "Margin calls" }))
    .getAllByRole("row")
    .slice(1);
  expect(schedule.map((row) => row.textContent?.slice(0, 20))).toEqual([
    "2024-06-172024-06-18",
    "2024-06-202024-06-21",
  ]);
  const dates = within(screen.getByRole("region", { name: "Call dates" }))
    .getAllByRole("listitem")
    .map((item) => item.textContent);
  expect(dates).toEqual(fixture.callDates.result);
  expect(dates).not.toContain("2024-06-19");
});
it("shows Rust's no-call decision and the empty, loading and native-error states", () => {
  const { rerender } = render(<MarginCallTable vm={report("below-mta")} />);
  expect(screen.getByText("No margin call")).toBeTruthy();
  expect(screen.queryByRole("region", { name: "Margin calls" })).toBeNull();
  rerender(<MarginCallTable calls={[]} />);
  expect(screen.getByText("No margin calls")).toBeTruthy();
  rerender(<MarginCallTable />);
  expect(screen.getByRole("status").textContent).toBe(
    "No variation margin result",
  );
  rerender(<MarginCallTable loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Calculating variation margin…",
  );
  rerender(<MarginCallTable error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
function Connected({ request }: { request: VmRequest }) {
  const vm = useVm(request);
  const schedule = useMarginCalls(callsRequest);
  const dates = useMarginCallDates(fixture.callDates.request);
  return (
    <MarginCallTable
      vm={vm.data ?? null}
      calls={schedule.data ?? null}
      callDates={dates.data ?? null}
      error={vm.error?.message}
    />
  );
}
it(
  "renders the worker results through the hooks, then the native currency error",
  async () => {
    const { unmount } = render(
      <FinstackQueryProvider>
        <Connected request={collect.request} />
      </FinstackQueryProvider>,
    );
    await waitFor(
      () =>
        expect(screen.getByLabelText("Net margin").textContent).toBe(
          "Net margin USD -1000000",
        ),
      { timeout: budget(5000) },
    );
    await waitFor(
      () =>
        expect(screen.getByRole("region", { name: "Call dates" })).toBeTruthy(),
      { timeout: budget(5000) },
    );
    unmount();
    render(
      <FinstackQueryProvider>
        <Connected
          request={{
            ...collect.request,
            csaJson: JSON.stringify({ ...csa, calendar_id: "nope" }),
          }}
        />
      </FinstackQueryProvider>,
    );
    await waitFor(
      () =>
        expect(screen.getByRole("alert").textContent).toContain(
          "calendar 'nope'",
        ),
      { timeout: budget(5000) },
    );
  },
  budget(15000),
);
function ConnectedEditor({ onSubmit }: { onSubmit(json: string): void }) {
  const validate = useCsaValidator();
  const loadPreset = useCsaPreset();
  return (
    <CsaEditor
      validate={validate}
      loadPreset={loadPreset}
      onSubmit={onSubmit}
    />
  );
}
it(
  "loads a Rust preset into the editor and submits the natively canonical CSA",
  async () => {
    const onSubmit = vi.fn();
    render(
      <FinstackQueryProvider>
        <ConnectedEditor onSubmit={onSubmit} />
      </FinstackQueryProvider>,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "EUR regulatory" }),
    );
    const submit = await screen.findByRole("button", { name: "Use CSA" });
    await waitFor(
      async () => {
        await userEvent.click(submit);
        expect(onSubmit).toHaveBeenCalled();
      },
      { timeout: budget(5000) },
    );
    expect(JSON.parse(onSubmit.mock.calls.at(-1)![0] as string)).toEqual(
      fixture.presets.eur,
    );
  },
  budget(15000),
);
it("shows the native preset error and keeps the regulatory inputs empty by default", async () => {
  const loadPreset = vi.fn(async (request: CsaPresetRequest) => {
    throw new Error(`rejected ${request.kind}`);
  });
  render(
    <CsaEditor
      validate={async (json) => json}
      loadPreset={loadPreset}
      onSubmit={() => {}}
    />,
  );
  const terms = within(
    screen.getByRole("form", { name: "Regulatory terms for a currency" }),
  );
  for (const label of ["Currency", "CSA id", "Collateral curve"])
    expect(terms.getByLabelText(label)).toHaveProperty("value", "");
  await userEvent.click(
    terms.getByRole("button", { name: "Load regulatory terms" }),
  );
  expect(loadPreset).toHaveBeenCalledWith({
    kind: "regulatory",
    currency: "",
    id: "",
    collateralCurve: "",
  });
  expect((await screen.findByRole("alert")).textContent).toBe(
    "rejected regulatory",
  );
});
it("hides the presets without a loader and starts from the Rust schema example", async () => {
  const onSubmit = vi.fn();
  render(<CsaEditor validate={async (json) => json} onSubmit={onSubmit} />);
  expect(
    screen.queryByRole("region", { name: "Regulatory presets" }),
  ).toBeNull();
  const submit = await screen.findByRole("button", { name: "Use CSA" });
  await waitFor(
    async () => {
      await userEvent.click(submit);
      expect(onSubmit).toHaveBeenCalled();
    },
    { timeout: budget(5000) },
  );
  expect(JSON.parse(onSubmit.mock.calls.at(-1)![0] as string)).toMatchObject({
    id: "CSA-ACME-2024",
    collateral_curve_id: "USD-OIS",
  });
});
