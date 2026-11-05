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
import { OptionCalculator } from "@/components/finstack/models/components/option-calculator/option-calculator";
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
  return envelope.value;
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
  const typed = {
    ...request,
    model: { ...request.model, df: Number.NaN },
  } as OptionCalculatorRequest;
  expect(await harness.proxy.optionCalculator(typed)).toEqual(envelope);
});
// Each solve recovers the volatility that priced it, which pins Rust's
// price dispatch against its implied-vol dispatch.
it.each([
  ["black_scholes", 0.2],
  ["black76", 0.25],
  ["bachelier", 0.0075],
])(
  "inverts the %s price back to the volatility that produced it",
  (model, vol) => {
    expect(value(`${model}_implied_vol`).implied_vol).toBeCloseTo(vol, 10);
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
    const base = request.model as unknown as Record<string, number>;
    const vol = (request.solve as { vol: number }).vol;
    const priced = async (change: Record<string, number>, at = vol) =>
      unwrap(
        await harness.proxy.optionCalculator({
          model: { ...request.model, ...change },
          solve: { kind: "price", vol: at },
        } as OptionCalculatorRequest),
      ).price;
    const { greeks } = value(`${model}_price`);
    const spot = base[underlying]!;
    const bump = spot / 10000;
    const delta =
      ((await priced({ [underlying]: spot + bump })) -
        (await priced({ [underlying]: spot - bump }))) /
      (2 * bump) /
      discount;
    expect(greeks.delta).toBeCloseTo(delta, 5);
    const volBump = vol / 10000;
    const vega =
      (((await priced({}, vol + volBump)) - (await priced({}, vol - volBump))) /
        (2 * volBump) /
        discount) *
      vegaUnit;
    expect(greeks.vega / vega).toBeCloseTo(1, 4);
  },
  budget(15000),
);
it("passes a theta denominator to Rust and leaves an omitted one unset", () => {
  const theta = (name: string) => {
    const { greeks } = value(name);
    if (greeks.kind !== "black_scholes")
      throw new Error("Black-Scholes Greeks expected");
    return greeks.theta;
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
function Wired({ start }: { start: OptionCalculatorRequest }) {
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
      <Wired start={request} />
    </FinstackQueryProvider>,
  );
  const result = value("bachelier_price");
  const tile = await screen.findByRole(
    "region",
    { name: "Price" },
    { timeout: budget(15000) },
  );
  expect(tile.textContent).toContain(sixPlaces(result.price));
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
      defaultRequest={request}
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
    model: {
      kind: "black76",
      is_call: false,
      forward: 100,
      strike: 100,
      df: 0.97,
      expiry: 1,
    },
    solve: { kind: "implied_vol", price: 4.5 },
  });
});
it("sends an empty field as NaN and a theta denominator only when entered", async () => {
  const onSubmit = vi.fn();
  const { request } = cases.black_scholes_price!;
  render(
    <OptionCalculator
      defaultRequest={request}
      onSubmit={onSubmit}
      result={null}
    />,
  );
  await userEvent.clear(input(/^Spot/));
  await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
  const first = onSubmit.mock.lastCall![0];
  expect(first.model.spot).toBeNaN();
  expect(first.model).not.toHaveProperty("theta_days_per_year");
  await userEvent.type(input(/^Theta days per year/), "252");
  await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
  expect(onSubmit.mock.lastCall![0].model.theta_days_per_year).toBe(252);
});
it("states the answered request's conventions, not the current selection", async () => {
  render(
    <OptionCalculator
      defaultRequest={cases.black76_price!.request}
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
      defaultRequest={cases.black_scholes_price!.request}
      onSubmit={() => undefined}
      result={result}
    />,
  );
  const greeks = screen.getByRole("region", { name: "Greeks" });
  const rows = within(greeks).getAllByRole("row");
  expect(rows).toHaveLength(7);
  expect(rows[1]!.textContent).toContain(String(result.greeks.delta));
  expect(greeks.textContent).toContain("per 1% move");
});
it("shows an implied volatility under the model's own name", () => {
  render(
    <OptionCalculator
      defaultRequest={cases.bachelier_implied_vol!.request}
      onSubmit={() => undefined}
      result={value("bachelier_implied_vol")}
      formatValue={sixPlaces}
    />,
  );
  const tile = screen.getByRole("region", {
    name: "Implied normal volatility",
  });
  expect(tile.textContent).toContain(
    sixPlaces(value("bachelier_implied_vol").implied_vol),
  );
});
it("fails the whole calculation when Rust rejects the Greeks at zero volatility", () => {
  const { envelope } = cases.zero_vol_rejects_greeks!;
  if (envelope.ok) throw new Error("Rust should reject these Greeks");
  render(
    <OptionCalculator
      defaultRequest={cases.zero_vol_rejects_greeks!.request}
      onSubmit={() => undefined}
      result={null}
      error={envelope.error.message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(envelope.error.message);
});
it("surfaces the native error of a price below intrinsic value, untouched", () => {
  const { envelope } = cases.below_intrinsic!;
  if (envelope.ok) throw new Error("Rust should reject this price");
  render(
    <OptionCalculator
      defaultRequest={cases.below_intrinsic!.request}
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
      defaultRequest={request}
      onSubmit={() => undefined}
      result={null}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("No option result");
  rerender(
    <OptionCalculator
      defaultRequest={request}
      onSubmit={() => undefined}
      result={null}
      loading
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("Calculating option…");
});
