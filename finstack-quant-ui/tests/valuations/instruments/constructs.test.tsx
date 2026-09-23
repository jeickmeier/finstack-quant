// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import * as bond from "../../../src/generated/instrument/bond";
import * as equity from "../../../src/generated/instrument/equity";
import * as structuredCredit from "../../../src/generated/instrument/structured_credit";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
) as { validateInstrumentJson(json: string): string };
const validate = vi.fn(async (json: string) =>
  native.validateInstrumentJson(json),
);
afterEach(() => {
  cleanup();
  validate.mockClear();
});
async function apply(submit: ReturnType<typeof vi.fn>) {
  const button = screen.getByRole("button", {
    name: "Apply instrument",
  }) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  fireEvent.submit(button.closest("form")!);
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  const result = submit.mock.calls[0][0] as string;
  expect(native.validateInstrumentJson(result)).toBe(result);
  submit.mockClear();
  return JSON.parse(result);
}
it("edits native map keys literally, rejects duplicates and removes entries", async () => {
  const user = userEvent.setup(),
    submit = vi.fn();
  render(
    <SchemaForm
      module={bond}
      validate={validate}
      onSubmit={submit}
      layout="full"
      fields={{ allow: ["instrument.spec.attributes"] }}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Add meta" }));
  await user.click(screen.getByRole("button", { name: "Add Meta" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Meta key 1" }), {
    target: { value: "desk.rate[0]/~" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Meta value 1" }), {
    target: { value: "0.00000000000001" },
  });
  const first = await apply(submit);
  expect(first.instrument.spec.attributes.meta).toEqual({
    "desk.rate[0]/~": "0.00000000000001",
  });
  await user.click(screen.getByRole("button", { name: "Add Meta" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Meta key 2" }), {
    target: { value: "desk.rate[0]/~" },
  });
  await waitFor(() =>
    expect(
      screen
        .getAllByRole("alert")
        .map((n) => n.textContent)
        .join(" "),
    ).toContain("Duplicate"),
  );
  const attempts = submit.mock.calls.length;
  fireEvent.submit(
    screen.getByRole("button", { name: "Apply instrument" }).closest("form")!,
  );
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("alert", { name: "Validation errors" }),
    ),
  );
  expect(submit).toHaveBeenCalledTimes(attempts);
  await user.click(screen.getByRole("button", { name: "Remove Meta 2" }));
  const final = await apply(submit);
  expect(final.instrument.spec.attributes.meta).toEqual(
    first.instrument.spec.attributes.meta,
  );
});
it("adds and edits a native tuple array and distinguishes nullable omission", async () => {
  const user = userEvent.setup(),
    submit = vi.fn();
  render(
    <SchemaForm
      module={equity}
      validate={validate}
      onSubmit={submit}
      layout="full"
      fields={{
        allow: [
          "instrument.spec.discrete_dividends",
          "instrument.spec.price_quote",
        ],
      }}
    />,
  );
  await user.click(
    screen.getByRole("button", { name: "Add Discrete dividends" }),
  );
  const date = screen.getByLabelText("Discrete dividends 1 1", {
    selector: "input",
  });
  fireEvent.change(date, { target: { value: "2025-06-01" } });
  fireEvent.change(
    screen.getByRole("textbox", { name: "Discrete dividends 1 2" }),
    { target: { value: "1.23456789" } },
  );
  await user.click(screen.getByRole("button", { name: "Add price quote" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Price quote" }), {
    target: { value: "123.456789" },
  });
  const first = await apply(submit);
  expect(first.instrument.spec.discrete_dividends).toEqual([
    ["2025-06-01", 1.23456789],
  ]);
  expect(first.instrument.spec.price_quote).toBe(123.456789);
  await user.click(screen.getByRole("button", { name: "Price quote options" }));
  await user.click(
    await screen.findByRole("button", { name: "Clear price quote" }),
  );
  const cleared = await apply(submit);
  expect(cleared.instrument.spec.price_quote).toBeNull();
  await user.click(screen.getByRole("button", { name: "Omit price quote" }));
  await apply(submit);
  expect(
    JSON.parse(validate.mock.calls.at(-1)![0]).instrument.spec,
  ).not.toHaveProperty("price_quote");
  await user.click(
    screen.getByRole("button", {
      name: "Set price quote to null",
    }),
  );
  const nulled = await apply(submit);
  expect(nulled.instrument.spec.price_quote).toBeNull();
});
it("inserts a nullable object and edits a recursive union inside its tuple through native validation", async () => {
  const user = userEvent.setup(),
    submit = vi.fn();
  render(
    <SchemaForm
      module={structuredCredit}
      validate={validate}
      onSubmit={submit}
      layout="full"
      fields={{ allow: ["instrument.spec.waterfall_rules"] }}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Add waterfall rules" }));
  await user.click(screen.getByRole("button", { name: "Add reserve" }));
  await user.click(
    within(screen.getByRole("radiogroup", { name: "Target type" })).getByRole(
      "radio",
      { name: "Max" },
    ),
  );
  await user.click(
    within(screen.getByRole("radiogroup", { name: "Max 1 type" })).getByRole(
      "radio",
      { name: "Pct of current" },
    ),
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Pct of current" }), {
    target: { value: "0.02" },
  });
  await user.click(
    within(screen.getByRole("radiogroup", { name: "Max 2 type" })).getByRole(
      "radio",
      { name: "Pct of original" },
    ),
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Pct of original" }), {
    target: { value: "0.03" },
  });
  const result = await apply(submit);
  expect(result.instrument.spec.waterfall_rules.reserve.target).toEqual({
    max: [{ pct_of_current: 0.02 }, { pct_of_original: 0.03 }],
  });
});
it("edits a union payload inside a native array of tranche objects", async () => {
  const submit = vi.fn();
  render(
    <SchemaForm
      module={structuredCredit}
      validate={validate}
      onSubmit={submit}
      layout="full"
      fields={{ allow: ["instrument.spec.tranches.tranches[0].coupon"] }}
    />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Rate" }), {
    target: { value: "0.065" },
  });
  const result = await apply(submit);
  expect(result.instrument.spec.tranches.tranches[0].coupon).toEqual({
    fixed: { rate: 0.065 },
  });
});
