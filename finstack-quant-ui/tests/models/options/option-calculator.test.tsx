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
  optionCalculatorOptions,
  useOptionCalculator,
  type OptionCalculatorRequest,
  type OptionCalculatorResult,
} from "@/hooks/models/use-option-calculator/use-option-calculator";
import {
  OptionCalculator,
  type OptionCalculation,
  type OptionCalculationResult,
} from "@/components/finstack/models/components/option-calculator/option-calculator";
import { unwrap, type Envelope } from "@/workers/finstack-contract";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness(["optionCalculator"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
type Case = {
  request: OptionCalculatorRequest;
  envelope: Envelope<OptionCalculatorResult>;
};
const cases = fixture as unknown as Record<string, Case>;
const value = (name: string) => {
  const { envelope } = cases[name]!;
  if (!envelope.ok) throw new Error(`${name} is a native rejection`);
  return envelope.value as OptionCalculatorResult & OptionCalculationResult;
};
// Results are regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/options/generate.mjs).
const { empty_field: emptyField, ...callable } = cases;
it.each(Object.keys(callable))(
  "returns the %s native envelope through the real worker",
  async (name) => {
    const { request, envelope } = cases[name]!;
    expect(await harness.proxy.optionCalculator(request)).toEqual(envelope);
  },
  budget(15000),
);
it("rejects an empty field in Rust, which sees NaN", async () => {
  const { request, envelope } = emptyField!;
  const typed = { ...request, df: Number.NaN } as OptionCalculatorRequest;
  expect(await harness.proxy.optionCalculator(typed)).toEqual(envelope);
});
// The facade takes every term positionally and every term is a number, so
// TypeScript cannot catch a swapped pair. Each solve recovers the volatility
// that priced it, which pins the price call against the implied-vol call.
it.each([
  ["black_scholes", 0.2],
  ["black76", 0.25],
  ["bachelier", 0.0075],
])(
  "inverts the %s price back to the volatility that produced it",
  (model, vol) => {
    expect(value(`${model}_implied_vol`).mode).toBe("implied_vol");
    const solved = value(`${model}_implied_vol`) as { impliedVol: number };
    expect(solved.impliedVol).toBeCloseTo(vol, 10);
  },
);
// A central difference of the returned price pins each Greeks call against the
// price call, with no reference value: delta in the underlying, vega in vol.
it.each([
  // Black-Scholes vega is per 1% move and Black-76 price carries the discount.
  ["black_scholes", "spot", 1, 0.01],
  ["black76", "forward", 0.97, 1],
  ["bachelier", "forward", 1, 1],
] as const)(
  "agrees the %s Greeks with central differences of its price",
  async (model, underlying, discount, vegaUnit) => {
    const { request } = cases[`${model}_price`]!;
    const priced = async (change: Record<string, number>) =>
      (
        unwrap(
          await harness.proxy.optionCalculator({
            ...request,
            ...change,
          } as OptionCalculatorRequest),
        ) as { price: number }
      ).price;
    const base = request as unknown as Record<string, number>;
    const { greeks } = value(`${model}_price`);
    if (!greeks.ok) throw new Error("Greeks rejected");
    const spot = base[underlying]!;
    const bump = spot / 10000;
    const delta =
      ((await priced({ [underlying]: spot + bump })) -
        (await priced({ [underlying]: spot - bump }))) /
      (2 * bump) /
      discount;
    expect(greeks.value.delta).toBeCloseTo(delta, 5);
    const vol = base.vol!;
    const volBump = vol / 10000;
    const vega =
      (((await priced({ vol: vol + volBump })) -
        (await priced({ vol: vol - volBump }))) /
        (2 * volBump) /
        discount) *
      vegaUnit;
    expect(greeks.value.vega / vega).toBeCloseTo(1, 4);
  },
  budget(15000),
);
it("passes a theta denominator to Rust and leaves an omitted one unset", () => {
  const theta = (name: string) => {
    const { greeks } = value(name);
    if (!greeks.ok || !("theta" in greeks.value))
      throw new Error("Black-Scholes Greeks expected");
    return greeks.value.theta;
  };
  expect(theta("theta_trading_days")).not.toBe(theta("black_scholes_price"));
});
it("keys the query on the complete request and waits for one", () => {
  const { request } = cases.black76_price!;
  const options = optionCalculatorOptions(null, 3, request);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "optionCalculator",
    request,
  ]);
  expect(optionCalculatorOptions(null, 3, null).enabled).toBe(false);
});
const sixPlaces = (number: number) => number.toFixed(6);
const input = (label: RegExp | string) =>
  screen.getByRole("textbox", { name: label });
function Wired({ start }: { start: OptionCalculation }) {
  const query = useOptionCalculator(start);
  return (
    <OptionCalculator
      defaultRequest={start}
      onSubmit={() => undefined}
      result={query.data ?? null}
      loading={query.isFetching}
      error={query.error?.message}
      formatValue={sixPlaces}
    />
  );
}
it("shows the worker's price and Greeks through the hook", async () => {
  const { request } = cases.bachelier_price!;
  render(
    <FinstackQueryProvider>
      <Wired start={request as OptionCalculation} />
    </FinstackQueryProvider>,
  );
  const result = value("bachelier_price");
  const tile = await screen.findByRole(
    "region",
    { name: "Price" },
    { timeout: budget(15000) },
  );
  expect(tile.textContent).toContain(
    sixPlaces((result as { price: number }).price),
  );
  expect(tile.textContent).toContain("Undiscounted");
  await waitFor(() =>
    expect(
      within(screen.getByRole("region", { name: "Greeks" })).getByRole("table"),
    ).toBeTruthy(),
  );
});
it("submits the named terms of the selected model and mode, untouched", async () => {
  const onSubmit = vi.fn();
  const { request } = cases.black_scholes_price!;
  render(
    <OptionCalculator
      defaultRequest={request as OptionCalculation}
      onSubmit={onSubmit}
      result={null}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(onSubmit).toHaveBeenLastCalledWith(request);
  await userEvent.click(screen.getByRole("radio", { name: "Black-76" }));
  await userEvent.click(
    screen.getByRole("radio", { name: "Implied volatility" }),
  );
  await userEvent.click(screen.getByRole("radio", { name: "Put" }));
  await userEvent.type(input(/^Forward/), "100");
  await userEvent.type(input(/^Discount factor/), "0.97");
  await userEvent.type(input(/^Option price/), "4.5");
  await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(onSubmit).toHaveBeenLastCalledWith({
    model: "black76",
    mode: "implied_vol",
    isCall: false,
    forward: 100,
    strike: 100,
    df: 0.97,
    expiry: 1,
    price: 4.5,
  });
});
it("sends an empty field as NaN and a theta denominator only when entered", async () => {
  const onSubmit = vi.fn();
  const { request } = cases.black_scholes_price!;
  render(
    <OptionCalculator
      defaultRequest={request as OptionCalculation}
      onSubmit={onSubmit}
      result={null}
    />,
  );
  await userEvent.clear(input(/^Spot/));
  await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
  const first = onSubmit.mock.lastCall![0];
  expect(first.spot).toBeNaN();
  expect(first).not.toHaveProperty("thetaDaysPerYear");
  await userEvent.type(input(/^Theta days per year/), "252");
  await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(onSubmit.mock.lastCall![0].thetaDaysPerYear).toBe(252);
});
it("states the returned model's conventions, not the current selection", async () => {
  render(
    <OptionCalculator
      defaultRequest={cases.black_scholes_price!.request as OptionCalculation}
      onSubmit={() => undefined}
      result={value("black76_price")}
      formatValue={sixPlaces}
    />,
  );
  await userEvent.click(screen.getByRole("radio", { name: "Bachelier" }));
  expect(screen.getByRole("region", { name: "Price" }).textContent).toContain(
    "Discounted",
  );
  const greeks = screen.getByRole("region", { name: "Greeks" });
  expect(within(greeks).getAllByRole("row")).toHaveLength(4);
  expect(greeks.textContent).toContain("vega per unit");
});
it("lists Black-Scholes Greeks in Rust's order with their units", () => {
  const result = value("black_scholes_price");
  render(
    <OptionCalculator
      defaultRequest={cases.black_scholes_price!.request as OptionCalculation}
      onSubmit={() => undefined}
      result={result}
    />,
  );
  const greeks = screen.getByRole("region", { name: "Greeks" });
  const rows = within(greeks).getAllByRole("row");
  expect(rows).toHaveLength(7);
  if (!result.greeks.ok) throw new Error("Greeks rejected");
  expect(rows[1]!.textContent).toContain(String(result.greeks.value.delta));
  expect(greeks.textContent).toContain("per 1% move");
});
it("shows an implied volatility under the model's own name", () => {
  render(
    <OptionCalculator
      defaultRequest={cases.bachelier_implied_vol!.request as OptionCalculation}
      onSubmit={() => undefined}
      result={value("bachelier_implied_vol")}
      formatValue={sixPlaces}
    />,
  );
  const tile = screen.getByRole("region", {
    name: "Implied normal volatility",
  });
  expect(tile.textContent).toContain(
    sixPlaces(
      (value("bachelier_implied_vol") as { impliedVol: number }).impliedVol,
    ),
  );
});
it("shows Greeks Rust rejects beside the price it returned", () => {
  const result = value("zero_vol_rejects_greeks");
  render(
    <OptionCalculator
      defaultRequest={
        cases.zero_vol_rejects_greeks!.request as OptionCalculation
      }
      onSubmit={() => undefined}
      result={result}
      formatValue={sixPlaces}
    />,
  );
  expect(screen.getByRole("region", { name: "Price" }).textContent).toContain(
    sixPlaces((result as { price: number }).price),
  );
  if (result.greeks.ok) throw new Error("Rust should reject these Greeks");
  expect(screen.getByRole("alert").textContent).toBe(
    result.greeks.error.message,
  );
});
it("surfaces the native error of a price below intrinsic value, untouched", () => {
  const { envelope } = cases.below_intrinsic!;
  if (envelope.ok) throw new Error("Rust should reject this price");
  render(
    <OptionCalculator
      defaultRequest={cases.below_intrinsic!.request as OptionCalculation}
      onSubmit={() => undefined}
      result={null}
      error={envelope.error.message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(envelope.error.message);
  expect(envelope.error.message).toContain("intrinsic");
});
it("waits and reports loading before any result", () => {
  const { request } = cases.black76_price!;
  const { rerender } = render(
    <OptionCalculator
      defaultRequest={request as OptionCalculation}
      onSubmit={() => undefined}
      result={null}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("No option result");
  rerender(
    <OptionCalculator
      defaultRequest={request as OptionCalculation}
      onSubmit={() => undefined}
      result={null}
      loading
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("Calculating option…");
});
