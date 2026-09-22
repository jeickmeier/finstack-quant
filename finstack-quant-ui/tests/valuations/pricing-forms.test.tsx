// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { useState } from "react";
import {
  cleanup,
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import { instruments } from "../../src/generated/instruments";
import { InstrumentForm } from "@/components/finstack/valuations/components/instrument-form/instrument-form";
import {
  PricingParamsForm,
  type PricingParams,
} from "@/components/finstack/valuations/components/pricing-params-form/pricing-params-form";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const validate = async (json: string) => native.validateInstrumentJson(json);
afterEach(cleanup);
it("loads bond lazily, replaces edits only on explicit example load, and focuses the canonical error path", async () => {
  const loader = vi.spyOn(
    instruments.find((x) => x.type === "bond")!,
    "loader",
  );
  const accepted = vi.fn();
  render(
    <InstrumentForm
      type="bond"
      onTypeChange={() => {}}
      validate={validate}
      onSubmit={() => {}}
      onValidated={accepted}
    />,
  );
  expect(screen.getByRole("status").textContent).toMatch(/Loading instrument/);
  const amount = await screen.findByRole("textbox", { name: "Amount" });
  expect(loader).toHaveBeenCalledTimes(1);
  const original = (amount as HTMLInputElement).value;
  fireEvent.focus(amount);
  fireEvent.change(amount, { target: { value: "abc" } });
  const error = await screen.findByRole("button", {
    name: "instrument.spec.notional.amount",
  });
  await userEvent.click(error);
  expect(document.activeElement).toBe(amount);
  expect(accepted).toHaveBeenCalledWith(null);
  fireEvent.change(amount, { target: { value: "1234567.123456789" } });
  await waitFor(() =>
    expect(accepted.mock.calls.at(-1)?.[0]).toContain("1234567.123456789"),
  );
  await userEvent.click(screen.getByRole("button", { name: "Load example" }));
  expect(
    (
      (await screen.findByRole("textbox", {
        name: "Amount",
      })) as HTMLInputElement
    ).value,
  ).toBe(original);
  loader.mockRestore();
});
it("loads a selected non-bond schema and lists the complete catalogue", async () => {
  const entry = instruments.find((x) => x.type === "equity")!;
  const loader = vi.spyOn(entry, "loader");
  render(
    <InstrumentForm
      type="equity"
      onTypeChange={() => {}}
      validate={validate}
      onSubmit={() => {}}
    />,
  );
  expect(
    await screen.findByRole("button", { name: "Load example" }),
  ).toBeTruthy();
  await userEvent.click(
    screen.getByRole("combobox", { name: "Instrument type" }),
  );
  await waitFor(() =>
    expect(screen.getAllByRole("option")).toHaveLength(instruments.length),
  );
  expect(loader).toHaveBeenCalledTimes(1);
  loader.mockRestore();
});
it("displays native semantic errors unchanged and retains edited text", async () => {
  render(
    <InstrumentForm
      type="bond"
      onTypeChange={() => {}}
      validate={async () => {
        throw new Error("Native validation: supplied curve is missing");
      }}
      onSubmit={() => {}}
    />,
  );
  await waitFor(() =>
    expect(
      screen
        .getAllByRole("alert")
        .map((x) => x.textContent)
        .join(" "),
    ).toContain("Native validation: supplied curve is missing"),
  );
  expect(await screen.findByRole("textbox", { name: "Amount" })).toBeTruthy();
});
it("preserves exact optional JSON, omission versus empty selection, hidden selections and complete controlled values", async () => {
  const history = '{"seed":18446744073709551615,"states":[]}';
  const initial: PricingParams = {
    asOf: "2025-01-01",
    model: "discounting",
    metrics: ["hidden_metric"],
    marketHistory: history,
  };
  const changes = vi.fn();
  function App() {
    const [value, set] = useState(initial);
    return (
      <PricingParamsForm
        value={value}
        onValueChange={(next) => {
          changes(next);
          set(next);
        }}
        instrumentType="bond"
        models={{ bond: ["discounting"], equity: ["equity_spot"] }}
        metrics={{ Risk: ["dv01"] }}
      />
    );
  }
  render(<App />);
  await userEvent.click(
    screen.getByRole("button", { name: "Add theta period" }),
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Theta period" }), {
    target: { value: "1W" },
  });
  await waitFor(() =>
    expect(changes.mock.calls.at(-1)?.[0].pricingOptions).toBe(
      '{"theta_period":"1W"}',
    ),
  );
  await userEvent.click(screen.getByRole("button", { name: "Metrics" }));
  await userEvent.click(screen.getByRole("checkbox", { name: /dv01/ }));
  expect(changes.mock.calls.at(-1)?.[0].metrics).toEqual([
    "hidden_metric",
    "dv01",
  ]);
  await userEvent.click(
    screen.getByRole("button", { name: "Omit metric selection" }),
  );
  expect(changes.mock.calls.at(-1)?.[0].metrics).toBeUndefined();
  await userEvent.click(screen.getByRole("button", { name: "Metrics" }));
  await userEvent.click(screen.getByRole("checkbox", { name: /dv01/ }));
  await userEvent.click(screen.getByRole("checkbox", { name: /dv01/ }));
  expect(changes.mock.calls.at(-1)?.[0].metrics).toEqual([]);
  expect(changes.mock.calls.at(-1)?.[0].marketHistory).toBe(history);
  expect(changes.mock.calls.at(-1)?.[0].pricingOptions).toBe(
    '{"theta_period":"1W"}',
  );
  const historyField = screen.getByRole("textbox", { name: "Market history" });
  fireEvent.change(historyField, { target: { value: "{" } });
  expect(historyField.getAttribute("aria-invalid")).toBe("true");
  expect((historyField as HTMLTextAreaElement).value).toBe("{");
  fireEvent.change(historyField, { target: { value: history } });
  await userEvent.click(
    screen.getByRole("button", { name: "Omit pricing overrides" }),
  );
  expect(changes.mock.calls.at(-1)?.[0].pricingOptions).toBeUndefined();
  await userEvent.click(screen.getByText("View market history"));
  await userEvent.click(
    within(
      screen.getByRole("region", { name: "Market history JSON" }),
    ).getByRole("button", { name: "Original" }),
  );
  expect(
    screen
      .getByRole("region", { name: "Market history JSON" })
      .querySelector("pre")?.textContent,
  ).toBe(history);
});
it("keeps a new structural error and invalid accepted state when an older native validation completes", async () => {
  let finish!: (json: string) => void;
  let old = "";
  const slow = vi.fn((json: string) => {
    old = json;
    return new Promise<string>((resolve) => {
      finish = resolve;
    });
  });
  const accepted = vi.fn();
  render(
    <InstrumentForm
      type="bond"
      onTypeChange={() => {}}
      validate={slow}
      onSubmit={() => {}}
      onValidated={accepted}
    />,
  );
  const amount = await screen.findByRole("textbox", { name: "Amount" });
  await waitFor(() => expect(slow).toHaveBeenCalledTimes(1));
  fireEvent.change(amount, { target: { value: "abc" } });
  await screen.findByRole("button", {
    name: "instrument.spec.notional.amount",
  });
  finish(native.validateInstrumentJson(old));
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Apply instrument",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  fireEvent.submit(amount.closest("form")!);
  await waitFor(() => expect(document.activeElement).toBe(amount));
  expect(amount.getAttribute("aria-invalid")).toBe("true");
  expect(accepted.mock.calls.at(-1)?.[0]).toBeNull();
});

it("waits for model discovery before announcing unavailable selection", () => {
  const props = {
    value: { asOf: "2025-01-01", model: "discounting" },
    onValueChange: () => {},
    instrumentType: "bond",
    models: {},
    metrics: {},
  };
  const { rerender } = render(
    <PricingParamsForm {...props} loading sections={["context"]} />,
  );
  expect(screen.queryByText(/Selected model unavailable/)).toBeNull();
  rerender(<PricingParamsForm {...props} sections={["context"]} />);
  expect(screen.getByText(/Selected model unavailable/)).toBeTruthy();
});
