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
import { termDisclosure } from "@/components/finstack/shared/components/schema-form/disclosure";
import { SchemaForm } from "@/components/finstack/shared/components/schema-form/schema-form";
import * as bond from "../../src/generated/instrument/bond";
import * as equityOption from "../../src/generated/instrument/equity_option";
import { createWireCodec } from "../../src/codec.mjs";
import { discriminator } from "@/components/finstack/shared/components/schema-form/discriminator";
import {
  initialValue,
  resolve,
  structuralValidator,
  workingValue,
  type Schema,
} from "@/components/finstack/shared/components/schema-form/schema";
import calibrationSchema from "../../src/generated/schemas/calibration.json";
afterEach(cleanup);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
) as { validateInstrumentJson(json: string): string };
const validate = async (json: string) => native.validateInstrumentJson(json);
function summaries(name: RegExp) {
  return [...document.querySelectorAll("summary")].filter((node) =>
    name.test(node.textContent ?? ""),
  );
}
it("keeps common bond terms on the sheet and conventions in collapsed disclosures", () => {
  expect(termDisclosure("day_count", { atSpec: true, siblingKeys: [] })).toBe(
    "Schedule conventions",
  );
  expect(
    termDisclosure("discount_curve_id", { atSpec: true, siblingKeys: [] }),
  ).toBe("Market links");
  expect(
    termDisclosure("spot_id", {
      atSpec: true,
      siblingKeys: ["underlying_ticker"],
    }),
  ).toBe("Market links");
  expect(
    termDisclosure("spot_id", { atSpec: true, siblingKeys: [] }),
  ).toBeNull();
  expect(
    termDisclosure("pool", {
      atSpec: true,
      instrumentType: "structured_credit",
      siblingKeys: [],
    }),
  ).toBe("Deal structure");
  expect(
    termDisclosure("fees", {
      atSpec: true,
      instrumentType: "revolving_credit",
      siblingKeys: [],
    }),
  ).toBe("Facility terms");
  expect(termDisclosure("rate", { atSpec: true, siblingKeys: [] })).toBeNull();
  expect(
    termDisclosure("id", {
      atSpec: true,
      instrumentType: "cds_option",
      siblingKeys: [],
    }),
  ).toBeNull();
});
it("edits generated bond controls, shows decimal errors and emits native canonical JSON without replacing edits", async () => {
  const submit = vi.fn();
  render(<SchemaForm module={bond} validate={validate} onSubmit={submit} />);
  const addCredit = screen.getByRole("button", { name: "Add credit curve id" });
  expect(addCredit.closest("details")?.open).toBe(false);
  expect(
    addCredit.closest("details")?.querySelector("summary")?.textContent,
  ).toMatch(/^Market links/);
  expect(screen.getByRole("textbox", { name: "Rate" })).toBeTruthy();
  expect(summaries(/^More /)).toEqual([]);
  const settlement = screen.getByRole("textbox", {
    name: "Settlement days",
  }) as HTMLInputElement;
  expect(settlement.closest("details")?.open).toBe(false);
  expect(settlement.value).toBe("1");
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
  expect(JSON.parse(canonical).instrument.spec.settlement_days).toBe(1);
  expect(
    JSON.parse(canonical).instrument.spec.cashflow_spec.fixed.day_count,
  ).toBe("act_act_isma");
  expect((amount as HTMLInputElement).value).toBe("1234567.1234567890");
});
it("keeps a defaulted exercise style on the equity option sheet", () => {
  render(
    <SchemaForm
      module={equityOption}
      validate={validate}
      onSubmit={() => {}}
    />,
  );
  const style = screen.getByRole("combobox", { name: "Exercise style" });
  expect(style.closest("details")).toBeNull();
});
it("opens a collapsed disclosure when its field is invalid", async () => {
  render(<SchemaForm module={bond} validate={validate} onSubmit={() => {}} />);
  const settlement = screen.getByRole("textbox", {
    name: "Settlement days",
  }) as HTMLInputElement;
  const details = settlement.closest("details");
  expect(details?.open).toBe(false);
  fireEvent.change(settlement, { target: { value: "nope" } });
  await waitFor(() =>
    expect(settlement.getAttribute("aria-invalid")).toBe("true"),
  );
  expect(details?.open).toBe(true);
  expect(document.activeElement).toBe(settlement);
  details!.open = false;
  details!.dispatchEvent(new Event("toggle", { bubbles: true }));
  expect(details?.open).toBe(true);
  expect(settlement.value).toBe("nope");
});
it("switches canonical external coupon branches, and exposes defaulted fields only on request", async () => {
  const user = userEvent.setup();
  render(<SchemaForm module={bond} validate={validate} onSubmit={() => {}} />);
  const dayCount = screen.getByRole("combobox", { name: "Day count" });
  expect(dayCount.closest("details")?.open).toBe(false);
  for (const summary of summaries(/^Schedule conventions/))
    await user.click(summary);
  expect(
    screen.getByRole("textbox", { name: "Settlement days" }).closest("details")
      ?.open,
  ).toBe(true);
  expect(dayCount.closest("details")?.open).toBe(true);
  const types = screen.getByRole("radiogroup", { name: "Cashflow spec type" });
  await user.click(within(types).getByRole("radio", { name: "Floating" }));
  expect(
    within(types)
      .getByRole("radio", { name: "Floating" })
      .getAttribute("aria-checked"),
  ).toBe("true");
  expect(screen.queryByRole("group", { name: "Fixed" })).toBeNull();
  await user.click(within(types).getByRole("radio", { name: "Fixed" }));
  expect(
    within(types)
      .getByRole("radio", { name: "Fixed" })
      .getAttribute("aria-checked"),
  ).toBe("true");
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
  fireEvent.click(summaries(/^Market links/)[0]!);
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
    screen.getByRole("button", { name: "Credit curve id options" }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Clear credit curve id" }),
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

it.each(["", "loaded"])(
  "keeps focus while editing between default and non-default values from %j",
  async (initial) => {
    const schema: Schema = {
      type: "object",
      properties: {
        terms: {
          type: "object",
          properties: { note: { type: "string", default: "" } },
        },
      },
    };
    const module = {
      schema,
      metadata: [],
      codec: createWireCodec({ ...schema }),
      example: { terms: { note: initial } },
    };
    const user = userEvent.setup();
    render(
      <SchemaForm
        module={module}
        validate={async (json) => json}
        onSubmit={() => {}}
      />,
    );
    if (!initial) await user.click(summaries(/^Less common terms/)[0]!);
    const note = screen.getByRole("textbox", { name: "Note" });
    if (initial) {
      await user.clear(note);
      expect(screen.getByRole("textbox", { name: "Note" })).toBe(note);
      expect(document.activeElement).toBe(note);
    }
    await user.type(note, "edited");
    expect(screen.getByRole("textbox", { name: "Note" })).toBe(note);
    expect(document.activeElement).toBe(note);
    expect((note as HTMLInputElement).value).toBe("edited");
  },
);

it("normalizes each immutable edit once across errors, native validation and submission", async () => {
  const codec = createWireCodec(bond.schema);
  const normalize = vi.spyOn(codec.validator, "parse");
  const stringify = vi.spyOn(codec, "stringify");
  const module = { ...bond, codec };
  const nativeValidate = vi.fn(validate);
  const submit = vi.fn();
  render(
    <SchemaForm module={module} validate={nativeValidate} onSubmit={submit} />,
  );
  await waitFor(() => expect(nativeValidate).toHaveBeenCalledTimes(1));
  expect(normalize).toHaveBeenCalledTimes(1);
  const input = screen.getByRole("textbox", {
    name: "Settlement days",
    hidden: true,
  });
  fireEvent.click(input.closest("details")!.querySelector("summary")!);
  fireEvent.change(input, { target: { value: "2" } });
  await waitFor(() => expect(nativeValidate).toHaveBeenCalledTimes(2));
  expect(normalize).toHaveBeenCalledTimes(2);
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  expect(normalize).toHaveBeenCalledTimes(2);
  expect(nativeValidate).toHaveBeenCalledTimes(2);
  expect(stringify).not.toHaveBeenCalled();
});
it("keeps undeclared boolean defaults unanswered while preserving declared defaults", () => {
  const schema: Schema = {
    type: "object",
    properties: {
      choice: { type: "boolean" },
      yes: { type: "boolean", default: true },
      no: { type: "boolean", default: false },
      optional: { type: "boolean" },
    },
    required: ["choice", "yes", "no"],
  };
  expect(initialValue(schema, { schema, pointer: "#" })).toEqual({
    choice: undefined,
    yes: true,
    no: false,
  });
});
it.each([true, false])(
  "requires an explicit %s choice and restores unanswered state on reset",
  async (choice) => {
    const schema: Schema = {
      type: "object",
      properties: { choice: { type: "boolean" } },
      required: ["choice"],
    };
    const module = {
      schema,
      metadata: [],
      codec: createWireCodec({ ...schema }),
      example: initialValue(schema, { schema, pointer: "#" }),
    };
    const validateChoice = vi.fn(async (json: string) => json);
    const submit = vi.fn();
    render(
      <SchemaForm
        module={module}
        validate={validateChoice}
        onSubmit={submit}
      />,
    );
    const checkbox = screen.getByRole("checkbox", { name: "Choice" });
    expect(checkbox.getAttribute("aria-checked")).toBe("mixed");
    await waitFor(() =>
      expect(checkbox.getAttribute("aria-invalid")).toBe("true"),
    );
    fireEvent.submit(checkbox.closest("form")!);
    expect(submit).not.toHaveBeenCalled();
    expect(validateChoice).not.toHaveBeenCalled();
    if (choice) fireEvent.click(checkbox);
    else
      fireEvent.click(
        screen.getByRole("button", { name: "Set choice to false" }),
      );
    await waitFor(() =>
      expect(validateChoice).toHaveBeenCalledWith(
        JSON.stringify({ choice }),
        expect.any(AbortSignal),
      ),
    );
    fireEvent.submit(checkbox.closest("form")!);
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith(JSON.stringify({ choice })),
    );
    fireEvent.click(screen.getByRole("button", { name: "Reset edits" }));
    await waitFor(() =>
      expect(
        screen
          .getByRole("checkbox", { name: "Choice" })
          .getAttribute("aria-checked"),
      ).toBe("mixed"),
    );
  },
);
it("requires an explicit is_cap answer in the generated cap_floor_vol schema", () => {
  const find = (node: unknown): Schema | undefined => {
    if (Array.isArray(node)) {
      for (const item of node) {
        const found = find(item);
        if (found) return found;
      }
      return undefined;
    }
    if (!node || typeof node !== "object") return undefined;
    const record = node as Record<string, unknown>;
    const properties = record.properties as
      Record<string, Schema | undefined> | undefined;
    if (properties?.cap_floor_vol) return properties.cap_floor_vol;
    for (const value of Object.values(record)) {
      const found = find(value);
      if (found) return found;
    }
    return undefined;
  };
  const capFloor = find(calibrationSchema);
  expect(capFloor?.required).toContain("is_cap");
  expect(capFloor?.properties?.is_cap?.type).toBe("boolean");
  expect(capFloor?.properties?.is_cap).not.toHaveProperty("default");
  const value = initialValue(calibrationSchema as Schema, {
    schema: capFloor!,
    pointer: "#",
  }) as Record<string, unknown>;
  expect(value.is_cap).toBeUndefined();
});

it("resolves escaped local references without mutating the input schema", () => {
  const root: Schema = {
    $defs: {
      "a/b~": { $ref: "#/$defs/N", title: "target" },
      N: { type: "number" },
      C: { $ref: "#/$defs/C" },
    },
    type: "object",
  };
  const before = JSON.stringify(root);
  const location = resolve(root, {
    schema: { $ref: "#/$defs/a~1b~0", description: "local" },
    pointer: "#",
  });
  expect(location.pointer).toBe("#/$defs/N");
  expect(location.schema.type).toBe("number");
  expect(location.schema.title).toBe("target");
  expect(location.schema.description).toBe("local");
  expect(() =>
    resolve(root, { schema: { $ref: "#/$defs/C" }, pointer: "#" }),
  ).toThrow("Unsupported schema reference");
  expect(JSON.stringify(root)).toBe(before);
});

it("workingValue keeps wide integer tokens and incomplete edit text exact", () => {
  const wide: Schema = { type: "integer", format: "int64" };
  expect(
    workingValue({}, { schema: wide, pointer: "#" }, "-9223372036854775808"),
  ).toBe(-9223372036854775808n);
  expect(
    workingValue(
      {},
      { schema: { type: "string" }, pointer: "#" },
      "9007199254740993.1234567890",
    ),
  ).toBe("9007199254740993.1234567890");
  expect(
    workingValue({}, { schema: { type: "integer" }, pointer: "#" }, "1."),
  ).toBe("1.");
  expect(
    workingValue({}, { schema: { type: "number" }, pointer: "#" }, "1e"),
  ).toBe("1e");
});
