// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import { SchemaForm } from "../registry/components/schema-form/schema-form";
import * as bond from "../src/generated/instrument/bond";
import { createWireCodec } from "../src/codec.mjs";
import { discriminator } from "../registry/components/schema-form/discriminator";
import {
  structuralValidator,
  type Schema,
} from "../registry/components/schema-form/schema";
afterEach(cleanup);
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
) as { validateInstrumentJson(json: string): string };
const validate = async (json: string) => native.validateInstrumentJson(json);
it("edits generated bond controls, shows decimal errors and emits native canonical JSON without replacing edits", async () => {
  const submit = vi.fn();
  render(<SchemaForm module={bond} validate={validate} onSubmit={submit} />);
  expect(screen.queryByRole("textbox", { name: "Settlement days" })).toBeNull();
  const amount = within(
    screen.getByRole("group", { name: "Notional" }),
  ).getByRole("textbox", { name: "Amount" });
  fireEvent.focus(amount);
  fireEvent.change(amount, { target: { value: "abc" } });
  await waitFor(() => expect(amount.getAttribute("aria-invalid")).toBe("true"));
  expect(
    screen
      .getAllByRole("alert")
      .map((node) => node.textContent)
      .join(" "),
  ).toMatch(/pattern|decimal/i);
  fireEvent.change(amount, { target: { value: "1234567.1234567890" } });
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
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  const canonical = submit.mock.calls[0][0];
  expect(native.validateInstrumentJson(canonical)).toBe(canonical);
  expect(JSON.parse(canonical).instrument.spec.notional.amount).toBe(
    "1234567.1234567890",
  );
  expect((amount as HTMLInputElement).value).toBe("1234567.1234567890");
});
it("switches canonical external coupon branches, and exposes defaulted fields only on request", async () => {
  const user = userEvent.setup();
  render(<SchemaForm module={bond} validate={validate} onSubmit={() => {}} />);
  await user.click(
    screen.getByRole("button", { name: "More instrument terms fields" }),
  );
  expect(screen.getByRole("textbox", { name: "Settlement days" })).toBeTruthy();
  const types = screen.getByRole("radiogroup", { name: "Cashflow spec type" });
  await user.click(within(types).getByRole("radio", { name: "Floating" }));
  expect(screen.getByRole("group", { name: "Floating" })).toBeTruthy();
  expect(screen.queryByRole("group", { name: "Fixed" })).toBeNull();
  await user.click(within(types).getByRole("radio", { name: "Fixed" }));
  expect(screen.getByRole("group", { name: "Fixed" })).toBeTruthy();
});
it("supports nullable references without inventing IDs and ignores stale native validation errors", async () => {
  const pending: {
    json: string;
    resolve(value: string): void;
    reject(error: Error): void;
  }[] = [];
  render(
    <SchemaForm
      module={bond}
      validate={(json) =>
        new Promise((resolve, reject) =>
          pending.push({ json, resolve, reject }),
        )
      }
      onSubmit={() => {}}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Add credit curve id" }));
  const credit = screen.getByRole("textbox", { name: "Credit curve id" });
  expect((credit as HTMLInputElement).value).toBe("");
  fireEvent.change(credit, { target: { value: "OLD" } });
  await waitFor(() => expect(pending.length).toBeGreaterThan(0));
  fireEvent.change(credit, { target: { value: "NEW" } });
  await waitFor(() => expect(pending.length).toBe(2));
  pending[1].resolve(native.validateInstrumentJson(pending[1].json));
  pending[0].reject(new Error("Obsolete native error"));
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Apply instrument",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  expect(screen.queryByText("Obsolete native error")).toBeNull();
  fireEvent.click(
    screen.getByRole("button", { name: "Clear credit curve id" }),
  );
  expect(screen.queryByRole("textbox", { name: "Credit curve id" })).toBeNull();
});
it("preserves real bond wide-integer fields until the generated codec checks their range", async () => {
  const edited = structuredClone(bond.example) as unknown as Record<
    string,
    unknown
  >;
  const spec = (edited.instrument as { spec: Record<string, unknown> }).spec;
  // Reference model inputs from valuations/benches/merton_mc_pricing.rs; no pricing is run here.
  const config = {
    seed: "18446744073709551615",
    merton: {
      asset_value: 200,
      asset_vol: 0.25,
      debt_barrier: 100,
      risk_free_rate: 0.04,
      payout_rate: 0,
      barrier_type: "terminal",
      dynamics: "geometric_brownian",
    },
    pik_schedule: { uniform: "pik" },
    num_paths: 10000,
    antithetic: true,
    time_steps_per_year: 12,
    barrier_crossing: "discrete",
    default_recovery_rate: 0.4,
  };
  spec.instrument_pricing_overrides = {
    model_config: { merton_mc_config: config },
  };
  const validator = structuralValidator(bond);
  const text = bond.codec.stringify(validator.parse(edited));
  expect(text).toContain('"seed":18446744073709551615');
  expect(native.validateInstrumentJson(text)).toContain(
    '"seed":18446744073709551615',
  );
  config.seed = "18446744073709551616";
  expect(validator.safeParse(edited).success).toBe(false);
  config.seed = "18446744073709551615";
  const submit = vi.fn();
  render(
    <SchemaForm
      module={bond}
      defaultValues={edited}
      fields={{
        allow: [
          "instrument.spec.instrument_pricing_overrides.model_config.merton_mc_config.seed",
        ],
      }}
      validate={validate}
      onSubmit={submit}
    />,
  );
  const seed = screen.getByRole("textbox", { name: "Seed" });
  expect(seed.getAttribute("inputmode")).toBe("numeric");
  fireEvent.change(seed, { target: { value: "9007199254740993" } });
  fireEvent.submit(seed.closest("form")!);
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  expect(submit.mock.calls[0][0]).toContain('"seed":9007199254740993');
  expect((seed as HTMLInputElement).value).toBe("9007199254740993");
});
it("does not drop data-bearing branches from mixed unit/external enums", () => {
  const schema: Schema = {
    oneOf: [
      { const: "cash", type: "string" },
      {
        type: "object",
        properties: {
          split: {
            type: "object",
            properties: { fraction: { type: "number" } },
            required: ["fraction"],
          },
        },
        required: ["split"],
      },
    ],
  };
  const tagged = discriminator(schema, { schema, pointer: "#" })!;
  expect(tagged.selected("cash")).toBe(0);
  expect(tagged.switchTo(1, "cash")).toEqual({ split: { fraction: "" } });
  expect(tagged.switchTo(0, { split: { fraction: 0.5 } })).toBe("cash");
});
it("adds, moves and removes array rows while filters retain hidden canonical values", async () => {
  const schema = {
    type: "object",
    properties: {
      rows: { type: "array", items: { type: "string" } },
      hidden: { type: "string" },
    },
    required: ["rows", "hidden"],
  };
  const module = {
    schema,
    metadata: [],
    codec: createWireCodec(schema),
    example: { rows: ["first", "second"], hidden: "retain" },
  };
  const submit = vi.fn();
  render(
    <SchemaForm
      module={module}
      fields={{ deny: ["hidden"] }}
      validate={async (json) => json}
      onSubmit={submit}
    />,
  );
  expect(screen.queryByRole("textbox", { name: "Hidden" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Move Rows 2 up" }));
  expect(
    (screen.getByRole("textbox", { name: "Rows 1" }) as HTMLInputElement).value,
  ).toBe("second");
  fireEvent.click(screen.getByRole("button", { name: "Remove Rows 2" }));
  fireEvent.click(screen.getByRole("button", { name: "Add Rows" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Rows 2" }), {
    target: { value: "third" },
  });
  fireEvent.submit(
    screen.getByRole("textbox", { name: "Rows 2" }).closest("form")!,
  );
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  expect(JSON.parse(submit.mock.calls[0][0])).toEqual({
    rows: ["second", "third"],
    hidden: "retain",
  });
});
it.each(["external", "internal", "adjacent"])(
  "uses %s discriminator representation from schema structure",
  (kind) => {
    const variants =
      kind === "external"
        ? ["fixed", "floating"].map((tag) => ({
            type: "object",
            properties: { [tag]: { type: "string" } },
            required: [tag],
          }))
        : ["fixed", "floating"].map((tag) => ({
            type: "object",
            properties: {
              coupon_kind: { const: tag },
              ...(kind === "adjacent"
                ? {
                    payload: {
                      type: "object",
                      properties: { id: { type: "string" } },
                      required: ["id"],
                    },
                  }
                : { id: { type: "string" } }),
            },
            required: ["coupon_kind", kind === "adjacent" ? "payload" : "id"],
          }));
    const schema: Schema = { oneOf: variants };
    const tagged = discriminator(schema, { schema, pointer: "#" })!;
    const value = tagged.switchTo(
      1,
      kind === "adjacent"
        ? { coupon_kind: "fixed", payload: { id: "keep" } }
        : { coupon_kind: "fixed", id: "keep" },
    );
    expect(tagged.selected(value)).toBe(1);
    expect(value).toEqual(
      kind === "external"
        ? { floating: "" }
        : kind === "adjacent"
          ? { coupon_kind: "floating", payload: { id: "keep" } }
          : { coupon_kind: "floating", id: "keep" },
    );
  },
);

it("delegates optional field presentation without recursively reapplying its override", async () => {
  const schema: Schema = {
    type: "object",
    properties: { note: { type: "string" } },
    additionalProperties: false,
  };
  const module = {
    schema,
    metadata: [],
    codec: createWireCodec({ ...schema }),
    example: { note: "Stored note" },
  };
  render(
    <SchemaForm
      module={module}
      validate={async (json) => json}
      onSubmit={() => {}}
      renderField={({ path, renderDefault }) =>
        path === "note" ? (
          <section aria-label="Deferred note">{renderDefault()}</section>
        ) : undefined
      }
    />,
  );
  expect(screen.getByLabelText("Note")).toBeTruthy();
  expect(screen.getAllByRole("region", { name: "Deferred note" })).toHaveLength(
    1,
  );
});
