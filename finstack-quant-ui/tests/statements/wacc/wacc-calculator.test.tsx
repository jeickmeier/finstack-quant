// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  useWacc,
  waccOptions,
  type WaccRequest,
} from "@/hooks/statements/use-wacc/use-wacc";
import { WaccCalculator } from "@/components/finstack/statements/components/wacc-calculator/wacc-calculator";
import { DcfAssumptionsForm } from "@/components/finstack/statements/components/dcf-assumptions-form/dcf-assumptions-form";
import { unwrap } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";

vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness(["wacc"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const { cases, errors } = fixture;
const field = (wire: string) =>
  screen.getByRole("textbox", { name: labelFor(wire) }) as HTMLInputElement;
const enter = (wire: string, value: string) =>
  fireEvent.change(field(wire), { target: { value } });
const calculate = () =>
  fireEvent.click(screen.getByRole("button", { name: "Calculate" }));
function Live({ start }: { start: WaccRequest }) {
  const [inputs, setInputs] = useState<WaccRequest | null>(null);
  const rate = useWacc(inputs);
  return (
    <WaccCalculator
      defaultInputs={start}
      onSubmit={setInputs}
      rate={rate.data ?? null}
      loading={rate.isFetching}
      error={rate.error?.message}
    />
  );
}
const renderLive = (start: WaccRequest) =>
  render(
    <FinstackQueryProvider>
      <Live start={start} />
    </FinstackQueryProvider>,
  );

it("returns each fixture rate through the worker", async () => {
  for (const entry of Object.values(cases))
    expect(unwrap(await harness.proxy.wacc(entry.inputs))).toBe(entry.rate);
});

it("shows the rate Rust returns for the entered decimals", async () => {
  renderLive(cases.blended.inputs);
  calculate();
  expect(
    await screen.findByText(String(cases.blended.rate), undefined, {
      timeout: budget(10000),
    }),
  ).toBeTruthy();
});

it("sends the two weights as entered, and shows Rust's rejection of their sum", async () => {
  renderLive(cases.blended.inputs);
  enter("debt_weight", String(errors.weights.inputs.debtWeight));
  calculate();
  await waitFor(
    () =>
      expect(screen.getByRole("alert").textContent).toBe(
        errors.weights.message,
      ),
    { timeout: budget(10000) },
  );
});

it("shows Rust's rejection of a tax rate out of range", async () => {
  renderLive(cases.blended.inputs);
  enter("tax_rate", String(errors.tax.inputs.taxRate));
  calculate();
  await waitFor(
    () =>
      expect(screen.getByRole("alert").textContent).toBe(errors.tax.message),
    { timeout: budget(10000) },
  );
});

it("submits an empty field as NaN, which Rust rejects, never as zero", async () => {
  renderLive(cases.blended.inputs);
  enter("equity_weight", "");
  calculate();
  await waitFor(
    () =>
      expect(screen.getByRole("alert").textContent).toBe(errors.empty.message),
    { timeout: budget(10000) },
  );
});

it("labels each input as a decimal and starts empty without defaults", () => {
  const onSubmit = vi.fn();
  render(<WaccCalculator onSubmit={onSubmit} rate={null} />);
  for (const wire of [
    "equity_weight",
    "cost_of_equity",
    "debt_weight",
    "cost_of_debt",
    "tax_rate",
  ])
    expect(field(wire).value).toBe("");
  expect(screen.getByText(/every value as a decimal/)).toBeTruthy();
  calculate();
  const [submitted] = onSubmit.mock.calls[0]!;
  expect(Object.values(submitted).every(Number.isNaN)).toBe(true);
});

it("offers apply only with the prop, and passes the rate on the click alone", () => {
  const onApply = vi.fn();
  const { rerender } = render(
    <WaccCalculator
      defaultInputs={cases.blended.inputs}
      onSubmit={() => {}}
      rate={cases.blended.rate}
    />,
  );
  expect(screen.queryByRole("button", { name: "Use as WACC" })).toBeNull();
  rerender(
    <WaccCalculator
      defaultInputs={cases.blended.inputs}
      onSubmit={() => {}}
      rate={cases.blended.rate}
      onApply={onApply}
    />,
  );
  expect(onApply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Use as WACC" }));
  expect(onApply).toHaveBeenCalledTimes(1);
  expect(onApply).toHaveBeenCalledWith(cases.blended.rate);
});

it("keeps a returned rate of zero a rate, not a missing one", () => {
  const onApply = vi.fn();
  render(
    <WaccCalculator
      defaultInputs={cases.zero.inputs}
      onSubmit={() => {}}
      rate={cases.zero.rate}
      onApply={onApply}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Use as WACC" }));
  expect(onApply).toHaveBeenCalledWith(0);
});

it("will not apply a rate after an edit since it was calculated", () => {
  const onApply = vi.fn();
  render(
    <WaccCalculator
      defaultInputs={cases.blended.inputs}
      onSubmit={() => {}}
      rate={cases.blended.rate}
      onApply={onApply}
    />,
  );
  enter("tax_rate", "0.3");
  const apply = screen.getByRole("button", {
    name: "Use as WACC",
  }) as HTMLButtonElement;
  expect(apply.disabled).toBe(true);
  expect(screen.getByRole("status").textContent).toMatch(/calculate again/);
  calculate();
  expect(apply.disabled).toBe(false);
});

it("hides the rate and the button while an error or a load is showing", () => {
  const { rerender } = render(
    <WaccCalculator
      onSubmit={() => {}}
      rate={cases.blended.rate}
      onApply={() => {}}
      error={errors.weights.message}
    />,
  );
  expect(screen.queryByRole("button", { name: "Use as WACC" })).toBeNull();
  rerender(
    <WaccCalculator
      onSubmit={() => {}}
      rate={cases.blended.rate}
      onApply={() => {}}
      loading
    />,
  );
  expect(screen.queryByRole("button", { name: "Use as WACC" })).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("Calculating…");
});

it("keys the query on the snapshot, with NaN distinct from null", () => {
  const client = { call: vi.fn() } as unknown as FinstackClient;
  const nan = { ...cases.blended.inputs, taxRate: Number.NaN };
  const empty = {
    ...cases.blended.inputs,
    taxRate: null,
  } as unknown as WaccRequest;
  expect(waccOptions(client, 1, nan).queryKey).not.toEqual(
    waccOptions(client, 1, empty).queryKey,
  );
  expect(waccOptions(client, 1, null).enabled).toBe(false);
});

it("lets a host set the DCF form's WACC without touching the other fields", () => {
  function Host() {
    const [wacc, setWacc] = useState("");
    return (
      <>
        <button
          type="button"
          onClick={() => setWacc(String(cases.blended.rate))}
        >
          Apply
        </button>
        <DcfAssumptionsForm
          nodeIds={[]}
          wacc={wacc}
          onWaccChange={setWacc}
          onSubmit={() => {}}
        />
      </>
    );
  }
  render(<Host />);
  const form = screen.getByRole("region", { name: "DCF assumptions" });
  const box = (name: string) =>
    within(form).getByRole("textbox", { name }) as HTMLInputElement;
  fireEvent.change(box("Net debt override"), { target: { value: "5" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(box("WACC").value).toBe(String(cases.blended.rate));
  expect(box("Net debt override").value).toBe("5");
  fireEvent.change(box("WACC"), { target: { value: "0.2" } });
  expect(box("WACC").value).toBe("0.2");
});
