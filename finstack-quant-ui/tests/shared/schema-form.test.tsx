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
import { LESS_COMMON_TERMS } from "@/components/finstack/core/components/schema-form/disclosure";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import * as bond from "../../src/generated/instrument/bond";
import { createWireCodec } from "../../src/codec.mjs";
import { discriminator } from "@/components/finstack/core/components/schema-form/discriminator";
import type { Schema } from "@/components/finstack/core/components/schema-form/schema";
import {
  initialValue,
  resolve,
  structuralValidator,
  workingValue,
} from "@/components/finstack/core/components/schema-form/schema-walk";
import calibrationSchema from "../../src/generated/schemas/calibration.json";
import checkSuiteSchema from "../../src/generated/schemas/check_suite_spec.json";
import checkSuiteMetadata from "../../src/generated/meta/check_suite_spec";
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
it("groups, roots and promotes fields through injected disclosure, sheet and promotedField", async () => {
  const schema: Schema = {
    type: "object",
    properties: {
      doc: {
        type: "object",
        properties: {
          terms: {
            type: "object",
            properties: {
              a: { type: "string" },
              b: { type: "string" },
              c: { type: "string" },
              note: { type: "string", default: "" },
            },
          },
        },
      },
    },
  };
  const module = {
    schema,
    metadata: [],
    codec: createWireCodec({ ...schema }),
    example: {
      doc: { terms: { a: "first", b: "second", c: "third", note: "" } },
    },
  };
  render(
    <SchemaForm
      module={module}
      validate={async (json) => json}
      onSubmit={() => {}}
      sheet={{ path: "doc", label: "Doc terms" }}
      disclosure={{
        order: ["B group", "C group", "A group"],
        group: ({ key, secondary }) =>
          key === "a"
            ? "A group"
            : key === "b"
              ? "B group"
              : key === "c"
                ? "C group"
                : secondary
                  ? LESS_COMMON_TERMS
                  : null,
      }}
      promotedField={({ reveal }) => ({
        path: "doc.terms.b",
        node: (
          <button type="button" onClick={() => reveal("doc.terms", "A group")}>
            Show A group
          </button>
        ),
      })}
    />,
  );
  // Listed groups follow `order` (C before A, against property order); the unlisted
  // generic group follows them. B's only member is the promoted, denied field, so
  // B is not rendered and no count includes it.
  expect(summaries(/./).map((node) => node.textContent)).toEqual([
    "C group (1)",
    "A group (1)",
    `${LESS_COMMON_TERMS} (1)`,
  ]);
  const root = screen.getByRole("group", { name: "Doc terms" });
  expect(root.classList.contains("finstack-schema-root")).toBe(true);
  expect(screen.queryByRole("group", { name: "Doc" })).toBeNull();
  expect(screen.queryByRole("textbox", { name: "B" })).toBeNull();
  const a = summaries(/^A group/)[0]!;
  expect((a.parentElement as HTMLDetailsElement).open).toBe(false);
  await userEvent.click(screen.getByRole("button", { name: "Show A group" }));
  expect(document.activeElement).toBe(a);
  expect((a.parentElement as HTMLDetailsElement).open).toBe(true);
});
it("preserves real bond wide-integer fields until the generated codec checks their range", async () => {
  const edited = structuredClone(bond.example) as unknown as Record<
    string,
    unknown
  >;
  const spec = (edited.instrument as { spec: Record<string, unknown> }).spec;
  // Reference model inputs from valuations/benches/merton_mc_pricing.rs; no pricing is run here.
  // The calibration seed is the Merton wire's u64 field.
  const calibration = {
    target: { clean_price_pct: 99.5 },
    parameter: "debt_barrier",
    low_paths: 2000,
    max_iterations: 40,
    tolerance_pv: 0.0001,
    bracket: null,
    seed: "18446744073709551615",
  };
  const config = {
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
    steps_per_year: 12,
    barrier_crossing: "discrete",
    recovery_rate: 0.4,
    calibration,
  };
  spec.instrument_pricing_overrides = {
    model_config: {
      mc_paths: 5000,
      mc_antithetic: true,
      merton_mc_config: config,
    },
  };
  const validator = structuralValidator(bond);
  const text = bond.codec.stringify(validator.parse(edited));
  expect(text).toContain('"seed":18446744073709551615');
  expect(native.validateInstrumentJson(text)).toContain(
    '"seed":18446744073709551615',
  );
  calibration.seed = "18446744073709551616";
  expect(validator.safeParse(edited).success).toBe(false);
  calibration.seed = "18446744073709551615";
  const submit = vi.fn();
  render(
    <SchemaForm
      module={bond}
      defaultValues={edited}
      fields={{
        allow: [
          "instrument.spec.instrument_pricing_overrides.model_config.merton_mc_config.calibration.seed",
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

it("merges a tagged struct variant's sibling tag into the referenced struct fields", () => {
  const root: Schema = {
    $defs: {
      S: {
        type: "object",
        properties: { nodes: { type: "array" }, tolerance: { type: "number" } },
        required: ["nodes"],
      },
    },
  };
  const before = JSON.stringify(root);
  const { schema, pointer } = resolve(root, {
    schema: {
      $ref: "#/$defs/S",
      properties: { type: { const: "a" } },
      required: ["type"],
    },
    pointer: "#",
  });
  expect(pointer).toBe("#/$defs/S");
  expect(Object.keys(schema.properties ?? {})).toEqual([
    "nodes",
    "tolerance",
    "type",
  ]);
  expect(schema.properties?.type?.const).toBe("a");
  expect(schema.required).toEqual(["nodes", "type"]);
  expect(JSON.stringify(root)).toBe(before);
});

it("edits a builtin check's struct fields through its generated tagged variant", async () => {
  const user = userEvent.setup();
  const module = {
    schema: checkSuiteSchema as Schema,
    metadata: checkSuiteMetadata,
    codec: createWireCodec(checkSuiteSchema),
    example: checkSuiteSchema.examples[0],
  };
  const input = vi.fn();
  render(
    <SchemaForm
      module={module}
      validate={async (json) => json}
      onSubmit={() => {}}
      onInputJson={input}
      layout="full"
    />,
  );
  await user.click(screen.getByRole("button", { name: "Add Builtin checks" }));
  await user.click(
    screen.getByRole("combobox", { name: "Builtin checks 2 type" }),
  );
  await user.click(
    await screen.findByRole("option", { name: "Balance sheet articulation" }),
  );
  const check = screen.getByRole("group", { name: "Builtin checks 2" });
  for (const name of ["Assets nodes", "Liabilities nodes", "Equity nodes"])
    expect(within(check).getByRole("group", { name })).toBeTruthy();
  await user.click(
    within(check).getByRole("button", { name: "Add tolerance" }),
  );
  await user.type(
    within(check).getByRole("textbox", { name: "Tolerance" }),
    "0.5",
  );
  await waitFor(() => {
    const json = input.mock.calls.at(-1)?.[0] as string | null | undefined;
    expect(json).toBeTruthy();
    expect(JSON.parse(json!).builtin_checks[1]).toEqual({
      type: "balance_sheet_articulation",
      assets_nodes: [],
      liabilities_nodes: [],
      equity_nodes: [],
      tolerance: 0.5,
    });
  });
  expect(input.mock.calls.at(-1)?.[0]).toContain('"tolerance":0.5');
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
