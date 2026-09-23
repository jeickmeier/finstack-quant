// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import { instruments } from "../../src/generated/instruments";
import { InstrumentForm } from "@/components/finstack/valuations/components/instrument-form/instrument-form";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const validate = async (json: string) => native.validateInstrumentJson(json);
afterEach(cleanup);

async function show(type: string, defaultJson?: string) {
  const accepted = vi.fn();
  render(
    <InstrumentForm
      type={type}
      onTypeChange={() => {}}
      defaultJson={defaultJson}
      validate={validate}
      onSubmit={() => {}}
      onValidated={accepted}
    />,
  );
  return accepted;
}

async function canonicalAfter(
  accepted: ReturnType<typeof vi.fn>,
  check: (spec: Record<string, unknown>) => boolean,
) {
  await waitFor(() => {
    const text = accepted.mock.calls.at(-1)?.[0];
    expect(typeof text).toBe("string");
    expect(check(JSON.parse(text).instrument.spec)).toBe(true);
  });
  return JSON.parse(accepted.mock.calls.at(-1)![0]).instrument.spec as Record<
    string,
    unknown
  >;
}

it("promotes the bond clean price, replaces only conflicting price quotes, and clears to omission", async () => {
  const module = await instruments
    .find((item) => item.type === "bond")!
    .loader();
  const example = structuredClone(module.example) as {
    instrument: { spec: Record<string, unknown> };
  };
  example.instrument.spec.instrument_pricing_overrides = {
    market_quotes: { quoted_ytm: 0.05, implied_volatility: 0.2 },
    model_config: { mc_paths: 1000 },
  };
  const accepted = await show("bond", module.codec.stringify(example));
  const input = await screen.findByRole("textbox", {
    name: "Bond clean price",
  });
  expect(
    screen.getAllByRole("textbox", { name: "Bond clean price" }),
  ).toHaveLength(1);
  expect(screen.getByText(/replaces Quoted YTM/)).toBeTruthy();
  await userEvent.click(
    screen.getByRole("button", { name: "More pricing overrides" }),
  );
  expect(
    (
      document.querySelector(
        "details[data-pricing-overrides]",
      ) as HTMLDetailsElement
    ).open,
  ).toBe(true);
  fireEvent.change(input, { target: { value: "99.5" } });
  const spec = await canonicalAfter(accepted, (spec) => {
    const quotes = ((
      spec.instrument_pricing_overrides as Record<string, unknown>
    )?.market_quotes ?? {}) as Record<string, unknown>;
    return quotes.quoted_clean_price === 99.5;
  });
  const quotes = (spec.instrument_pricing_overrides as Record<string, unknown>)
    .market_quotes as Record<string, unknown>;
  expect(quotes.quoted_ytm).toBeUndefined();
  expect(quotes.implied_volatility).toBe(0.2);
  expect(
    (
      spec.instrument_pricing_overrides as Record<
        string,
        Record<string, unknown>
      >
    ).model_config.mc_paths,
  ).toBe(1000);
  await userEvent.click(
    screen.getByRole("button", { name: "Clear bond clean price" }),
  );
  await canonicalAfter(accepted, (next) => {
    const market = ((
      next.instrument_pricing_overrides as Record<string, unknown>
    )?.market_quotes ?? {}) as Record<string, unknown>;
    const model = (
      next.instrument_pricing_overrides as Record<
        string,
        Record<string, unknown>
      >
    )?.model_config;
    return (
      market.quoted_clean_price === undefined &&
      market.implied_volatility === 0.2 &&
      model?.mc_paths === 1000
    );
  });
});

it("adds and removes a flat option volatility quote without duplicating its form field", async () => {
  const accepted = await show("equity_option");
  await canonicalAfter(
    accepted,
    (spec) => spec.instrument_pricing_overrides === undefined,
  );
  const input = await screen.findByRole("textbox", {
    name: "Implied volatility",
  });
  expect(
    screen.getAllByRole("textbox", { name: "Implied volatility" }),
  ).toHaveLength(1);
  expect(screen.getByText("Volatility surface")).toBeTruthy();
  fireEvent.change(input, { target: { value: "0.20" } });
  await canonicalAfter(accepted, (spec) => {
    const quotes = ((
      spec.instrument_pricing_overrides as Record<string, unknown>
    )?.market_quotes ?? {}) as Record<string, unknown>;
    return quotes.implied_volatility === 0.2;
  });
  fireEvent.change(input, { target: { value: "bad" } });
  await waitFor(() => expect(input.getAttribute("aria-invalid")).toBe("true"));
  fireEvent.change(input, { target: { value: "0.25" } });
  await canonicalAfter(accepted, (spec) => {
    const quotes = ((
      spec.instrument_pricing_overrides as Record<string, unknown>
    )?.market_quotes ?? {}) as Record<string, unknown>;
    return quotes.implied_volatility === 0.25;
  });
  await userEvent.click(
    screen.getByRole("button", { name: "Clear implied volatility" }),
  );
  await canonicalAfter(
    accepted,
    (spec) => spec.instrument_pricing_overrides === undefined,
  );
});

it.each([
  ["credit_default_swap", "Running spread", "premium", "spread_bp", "125"],
  ["interest_rate_swap", "Fixed coupon", "fixed", "rate", "0.045"],
  ["xccy_swap", "Leg 2 spread", "leg2", "spread_bp", "15"],
] as const)(
  "keeps the promoted %s deal term in the native instrument JSON",
  async (type, label, branch, key, text) => {
    const accepted = await show(type);
    const input = await screen.findByRole("textbox", { name: label });
    expect(screen.getAllByRole("textbox", { name: label })).toHaveLength(1);
    if (type === "xccy_swap")
      expect(
        screen.queryByRole("button", { name: "More pricing overrides" }),
      ).toBeNull();
    fireEvent.change(input, { target: { value: text } });
    await canonicalAfter(
      accepted,
      (spec) =>
        (spec[branch] as Record<string, unknown> | undefined)?.[key] === text,
    );
  },
);

it("clears the far FX outright back to a curve-sourced rate", async () => {
  const accepted = await show("fx_swap");
  const input = await screen.findByRole("textbox", { name: "Far FX rate" });
  expect((input as HTMLInputElement).value).toBe("1.12");
  await userEvent.click(
    screen.getByRole("button", { name: "Clear far fx rate" }),
  );
  await canonicalAfter(accepted, (spec) => spec.far_rate == null);
  expect(screen.getByText("Forward curves")).toBeTruthy();
});

it("promotes a single fixed tranche coupon with its tranche identity", async () => {
  const accepted = await show("structured_credit");
  const input = await screen.findByRole("textbox", { name: "Tranche coupon" });
  expect(screen.getByText("CLONOTES-A")).toBeTruthy();
  expect(screen.getByRole("region", { name: "Deal rate" })).toBeTruthy();
  fireEvent.change(input, { target: { value: "0.07" } });
  await canonicalAfter(
    accepted,
    (spec) =>
      ((
        spec.tranches as { tranches: { coupon: { fixed: { rate: number } } }[] }
      )?.tranches?.[0]?.coupon.fixed.rate ?? null) === 0.07,
  );
});

it("does not nominate the first tranche as a deal-wide quote when there are multiple", async () => {
  const module = await instruments
    .find((item) => item.type === "structured_credit")!
    .loader();
  const example = structuredClone(module.example) as {
    instrument: {
      spec: { tranches: { tranches: Record<string, unknown>[] } };
    };
  };
  const first = example.instrument.spec.tranches.tranches[0];
  example.instrument.spec.tranches.tranches.push({
    ...structuredClone(first),
    id: "CLONOTES-B",
  });
  await show("structured_credit", module.codec.stringify(example));
  await screen.findByRole("button", { name: "Load example" });
  expect(screen.queryByRole("region", { name: "Deal rate" })).toBeNull();
});
