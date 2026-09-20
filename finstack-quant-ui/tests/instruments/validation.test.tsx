// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { SchemaForm } from "../../registry/components/schema-form/schema-form";
import { issuePathToFieldPath } from "../../registry/components/schema-form/issue-mapping";
import {
  structuralValidator,
  type InstrumentModule,
} from "../../registry/components/schema-form/schema";
import * as bond from "../../src/generated/instrument/bond";
import * as deposit from "../../src/generated/instrument/deposit";
import * as equity from "../../src/generated/instrument/equity";
import * as fx from "../../src/generated/instrument/fx_option";
import * as cds from "../../src/generated/instrument/credit_default_swap";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
) as { validateInstrumentJson(json: string): string };
const validate = async (json: string) => native.validateInstrumentJson(json);
afterEach(cleanup);
function fixture(
  module: InstrumentModule,
  path: (string | number)[],
  value: unknown,
) {
  const data = structuredClone(module.example) as Record<string, any>;
  let target = data;
  for (const key of path.slice(0, -1)) target = target[key];
  target[path.at(-1)!] = value;
  return data;
}
it("keeps literal map keys, array indices and external tags in a single issue mapper", () => {
  const working = {
    rows: [
      { fixed: { values: [{ key: "desk.rate[0]/~1", value: [3, "bad"] }] } },
    ],
  };
  expect(
    issuePathToFieldPath(
      ["rows", 0, "fixed", "values", "desk.rate[0]/~1", 1],
      working,
    ),
  ).toEqual(["rows", 0, "fixed", "values", 0, "value", 1]);
  expect(
    issuePathToFieldPath(["rows", 0, "fixed", "values", "absent"], working),
  ).toBeUndefined();
  expect(issuePathToFieldPath(["bad.path"], {})).toBeUndefined();
  expect(
    issuePathToFieldPath(["values", "same"], {
      values: [
        { key: "same", value: 1 },
        { key: "same", value: 2 },
      ],
    }),
  ).toBeUndefined();
});
it.each([
  {
    name: "bond decimal",
    module: bond,
    path: ["instrument", "spec", "notional", "amount"],
    value: "bad",
    field: "instrument.spec.notional.amount",
  },
  {
    name: "deposit decimal",
    module: deposit,
    path: ["instrument", "spec", "quote_rate"],
    value: "bad",
    field: "instrument.spec.quote_rate",
  },
  {
    name: "bond external coupon",
    module: bond,
    path: ["instrument", "spec", "cashflow_spec", "fixed", "rate"],
    value: "bad",
    field: "instrument.spec.cashflow_spec.fixed.rate",
  },
  {
    name: "FX enum",
    module: fx,
    path: ["instrument", "spec", "option_type"],
    value: "bad",
    field: "instrument.spec.option_type",
  },
  {
    name: "equity dividend tuple",
    module: equity,
    path: ["instrument", "spec", "discrete_dividends"],
    value: [["2025-06-01", "bad"]],
    field: "instrument.spec.discrete_dividends[0][1]",
  },
  {
    name: "literal metadata map",
    module: bond,
    path: ["instrument", "spec", "attributes", "meta"],
    value: { "desk.rate[0]/~1": 12 },
    field: "instrument.spec.attributes.meta[0].value",
  },
])(
  "maps $name issues inline and focuses the control on submit",
  async ({ module, path, value, field }) => {
    const submit = vi.fn();
    const { container } = render(
      <SchemaForm
        module={module}
        defaultValues={fixture(module, path, value)}
        layout="full"
        validate={validate}
        onSubmit={submit}
      />,
    );
    const frame = container.querySelector(`[data-field-path="${field}"]`)!;
    expect(frame).not.toBeNull();
    await waitFor(() =>
      expect(frame.querySelector('[aria-invalid="true"]')).not.toBeNull(),
    );
    await waitFor(() =>
      expect(
        (container.querySelector('button[type="submit"]') as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    fireEvent.click(container.querySelector('button[type="submit"]')!);
    await waitFor(() =>
      expect(frame.contains(document.activeElement)).toBe(true),
    );
    expect(submit).not.toHaveBeenCalled();
  },
);
it("keeps unknown keys and hidden structural issues in the accessible summary", async () => {
  const defaults = fixture(
    bond,
    ["instrument", "spec", "notional", "amount"],
    "bad",
  );
  defaults.instrument.spec.unknown_registry_field = true;
  const submit = vi.fn();
  const { container } = render(
    <SchemaForm
      module={bond}
      defaultValues={defaults}
      fields={{ deny: ["instrument.spec.notional"] }}
      validate={validate}
      onSubmit={submit}
    />,
  );
  await waitFor(() =>
    expect(
      screen.getByRole("alert", { name: "Validation errors" }).textContent,
    ).toContain("unknown_registry_field"),
  );
  fireEvent.submit(container.querySelector("form")!);
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("alert", { name: "Validation errors" }),
    ),
  );
  expect(submit).not.toHaveBeenCalled();
});
it.each([
  {
    name: "bond maturity before issue",
    module: bond,
    path: ["instrument", "spec", "maturity"],
    value: "2020-01-01",
  },
  {
    name: "CDS recovery limit",
    module: cds,
    path: ["instrument", "spec", "protection", "recovery_rate"],
    value: 2,
  },
])(
  "retains the actual native $name diagnostic and focuses summary",
  async ({ module, path, value }) => {
    const defaults = fixture(module, path, value);
    expect(structuralValidator(module).safeParse(defaults).success).toBe(true);
    let message = "";
    try {
      native.validateInstrumentJson(module.codec.stringify(defaults));
    } catch (error) {
      message = (error as Error).message;
    }
    expect(message).not.toBe("");
    const submit = vi.fn();
    const { container } = render(
      <SchemaForm
        module={module}
        defaultValues={defaults}
        layout="full"
        validate={validate}
        onSubmit={submit}
      />,
    );
    await waitFor(() =>
      expect(
        screen.getByRole("alert", { name: "Validation errors" }).textContent,
      ).toContain(message),
    );
    fireEvent.click(container.querySelector('button[type="submit"]')!);
    await waitFor(() =>
      expect(document.activeElement).toBe(
        screen.getByRole("alert", { name: "Validation errors" }),
      ),
    );
    expect(submit).not.toHaveBeenCalled();
  },
);
it.each(["resolve", "reject"])(
  "discards a submitted native %s after a newer edit",
  async (outcome) => {
    const pending: {
      json: string;
      resolve(value: string): void;
      reject(error: Error): void;
    }[] = [];
    const submit = vi.fn();
    const { container } = render(
      <SchemaForm
        module={bond}
        validate={(json) =>
          new Promise((resolve, reject) =>
            pending.push({ json, resolve, reject }),
          )
        }
        onSubmit={submit}
      />,
    );
    fireEvent.submit(container.querySelector("form")!);
    await waitFor(() => expect(pending.length).toBeGreaterThan(0));
    const original = pending[0];
    const id = screen.getByRole("textbox", { name: "Id" });
    fireEvent.change(id, { target: { value: "CURRENT" } });
    if (outcome === "resolve")
      original.resolve(native.validateInstrumentJson(original.json));
    else original.reject(new Error("obsolete submit error"));
    await waitFor(() =>
      expect(pending.some((p) => p.json.includes("CURRENT"))).toBe(true),
    );
    for (const request of pending.slice(1))
      request.resolve(native.validateInstrumentJson(request.json));
    await waitFor(() =>
      expect(
        (container.querySelector('button[type="submit"]') as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    );
    expect(submit).not.toHaveBeenCalled();
    expect(screen.queryByText("obsolete submit error")).toBeNull();
  },
);
