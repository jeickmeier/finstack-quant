import { z } from "zod";
import { issuePathToFieldPath } from "./issue-mapping";
import { integerText, numericEdit, schemaAt } from "@/lib/finstack/schema.mjs";
import {
  propertiesOf,
  type InstrumentModule,
  type Schema,
  type SchemaLocation,
} from "./schema";

export function resolve(
  root: Schema,
  location: SchemaLocation,
): SchemaLocation {
  let { schema, pointer } = location;
  const seen = new Set<string>();
  while (schema.$ref) {
    const ref = schema.$ref;
    if (!ref.startsWith("#/") || seen.has(ref))
      throw new Error(`Unsupported schema reference: ${ref}`);
    seen.add(ref);
    const target = schemaAt(root, ref);
    if (!target || typeof target !== "object")
      throw new Error(`Missing schema reference: ${ref}`);
    const { $ref: _, ...siblings } = schema;
    schema = { ...(target as Schema), ...siblings };
    pointer = ref;
  }
  return { schema, pointer };
}
export function nullable(
  root: Schema,
  location: SchemaLocation,
): SchemaLocation | null {
  const { schema, pointer } = resolve(root, location);
  const variants = schema.anyOf ?? schema.oneOf;
  if (variants?.length === 2 && variants.some((node) => node.type === "null")) {
    const index = variants.findIndex((node) => node.type !== "null");
    return {
      schema: variants[index],
      pointer: `${pointer}/${schema.anyOf ? "anyOf" : "oneOf"}/${index}`,
    };
  }
  if (Array.isArray(schema.type) && schema.type.includes("null")) {
    // A null default is the omitted state. Add uses this branch and must
    // receive an editable value, not another null.
    const { default: fallback, ...rest } = schema;
    const next: Schema = {
      ...rest,
      type: schema.type.filter((type) => type !== "null")[0],
    };
    if (fallback != null) next.default = fallback;
    return { schema: next, pointer };
  }
  return null;
}
export const objectValue = (value: unknown): Record<string, unknown> =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
const ACRONYMS = new Set([
  "mc",
  "fx",
  "ois",
  "cds",
  "cms",
  "fra",
  "irs",
  "tba",
  "cmo",
  "mbs",
  "abs",
  "clo",
  "oas",
  "dv01",
  "cs01",
  "pv",
  "npv",
  "ytm",
  "atm",
  "rr",
  "bf",
  "hw1f",
  "cpr",
  "cdr",
  "wal",
  "isda",
  "imm",
  "sofr",
  "libor",
  "ndf",
  "trs",
  "yoy",
  "xccy",
  "vol",
  "sabr",
  "pde",
]);
/** Sentence-case labels from wire names; market acronyms stay upright. */
export const labelFor = (name: string) =>
  name
    .split("_")
    .map((word, index) =>
      ACRONYMS.has(word)
        ? word.toUpperCase()
        : index === 0
          ? word.replace(/^./, (letter) => letter.toUpperCase())
          : word,
    )
    .join(" ");

/** Create an editable branch from declared defaults; missing required inputs stay visibly incomplete. */
export function initialValue(
  root: Schema,
  location: SchemaLocation,
  depth = 0,
): unknown {
  if (depth > 20)
    throw new Error("Schema nesting exceeds the supported form depth");
  const { schema, pointer } = resolve(root, location);
  if (Object.hasOwn(schema, "const")) return schema.const;
  if (Object.hasOwn(schema, "default"))
    return editValue(
      root,
      { schema, pointer },
      structuredClone(schema.default),
    );
  if (nullable(root, { schema, pointer })) return null;
  if (
    schema.type === "object" &&
    typeof schema.additionalProperties === "object"
  )
    return [];
  if (schema.type === "object")
    return Object.fromEntries(
      (schema.required ?? []).map((key) => [
        key,
        initialValue(
          root,
          {
            schema: schema.properties?.[key] ?? {},
            pointer: `${pointer}/properties/${key}`,
          },
          depth + 1,
        ),
      ]),
    );
  if (schema.type === "array")
    return (
      schema.prefixItems?.map((node, index) =>
        initialValue(
          root,
          { schema: node, pointer: `${pointer}/prefixItems/${index}` },
          depth + 1,
        ),
      ) ?? []
    );
  if (schema.type === "boolean") return undefined;
  return "";
}

/** Represent open maps as key/value rows so punctuation in keys never becomes a form path. */
export function editValue(
  root: Schema,
  location: SchemaLocation,
  value: unknown,
): unknown {
  if (value == null) return value;
  const { schema, pointer } = resolve(root, location);
  const nonNull = nullable(root, { schema, pointer });
  if (nonNull) return editValue(root, nonNull, value);
  const branches = schema.oneOf ?? schema.anyOf;
  if (branches) {
    const branch = branches.find((node) => matchesBranch(root, node, value));
    return branch ? editValue(root, { schema: branch, pointer }, value) : value;
  }
  if (
    schema.type === "object" &&
    (typeof value !== "object" || Array.isArray(value))
  )
    return value;
  if (
    schema.type === "object" &&
    typeof schema.additionalProperties === "object"
  )
    return Object.entries(objectValue(value)).map(([key, item]) => ({
      key,
      value: editValue(
        root,
        {
          schema: schema.additionalProperties as Schema,
          pointer: `${pointer}/additionalProperties`,
        },
        item,
      ),
    }));
  if (schema.type === "object")
    return Object.fromEntries(
      Object.entries(objectValue(value)).map(([key, item]) => [
        key,
        editValue(
          root,
          {
            schema: schema.properties?.[key] ?? {},
            pointer: `${pointer}/properties/${key}`,
          },
          item,
        ),
      ]),
    );
  if (schema.type === "array" && Array.isArray(value))
    return value.map((item, index) =>
      editValue(
        root,
        {
          schema: schema.prefixItems?.[index] ?? schema.items ?? {},
          pointer: `${pointer}/items`,
        },
        item,
      ),
    );
  return value;
}

/** Convert numeric edit text at validation/output boundaries; preserve working text and exact decimal strings. */
export function workingValue(
  root: Schema,
  location: SchemaLocation,
  value: unknown,
): unknown {
  const { schema, pointer } = resolve(root, location);
  if (value === null || value === undefined) return value;
  const nonNull = nullable(root, { schema, pointer });
  if (nonNull) return workingValue(root, nonNull, value);
  const variants = schema.oneOf ?? schema.anyOf;
  if (variants) {
    const match = variants.find((node) => matchesBranch(root, node, value));
    return match
      ? workingValue(root, { schema: match, pointer }, value)
      : value;
  }
  if (
    schema.type === "object" &&
    typeof schema.additionalProperties === "object" &&
    Array.isArray(value)
  ) {
    const keys = new Set<string>();
    return Object.fromEntries(
      value.map((entry) => {
        const row = objectValue(entry);
        if (typeof row.key !== "string" || keys.has(row.key))
          throw new Error(`Duplicate or invalid map key: ${String(row.key)}`);
        keys.add(row.key);
        return [
          row.key,
          workingValue(
            root,
            {
              schema: schema.additionalProperties as Schema,
              pointer: `${pointer}/additionalProperties`,
            },
            row.value,
          ),
        ];
      }),
    );
  }
  if (
    schema.type === "object" &&
    value &&
    typeof value === "object" &&
    !Array.isArray(value)
  )
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, item]) => item !== undefined)
        .map(([key, item]) => [
          key,
          workingValue(
            root,
            {
              schema:
                schema.properties?.[key] ??
                (typeof schema.additionalProperties === "object"
                  ? schema.additionalProperties
                  : {}),
              pointer: `${pointer}/properties/${key}`,
            },
            item,
          ),
        ]),
    );
  if (schema.type === "array" && Array.isArray(value))
    return value.map((item, index) =>
      workingValue(
        root,
        {
          schema: schema.prefixItems?.[index] ?? schema.items ?? {},
          pointer: `${pointer}/items`,
        },
        item,
      ),
    );
  if (
    typeof value === "string" &&
    (schema.type === "number" || schema.type === "integer")
  ) {
    if (
      schema.type === "integer" &&
      ["int64", "uint64"].includes(schema.format ?? "") &&
      integerText.test(value)
    )
      return BigInt(value);
    return numericEdit(value, schema.type === "integer");
  }
  return value;
}
/** Match tag structure only; the generated codec validates all actual branch values. */
export function matchesBranch(
  root: Schema,
  node: Schema,
  value: unknown,
): boolean {
  const { schema } = resolve(root, { schema: node, pointer: "#" });
  if (Object.hasOwn(schema, "const")) return schema.const === value;
  if (schema.enum) return schema.enum.includes(value);
  if (schema.type === "null") return value === null;
  const object = objectValue(value);
  const constants = propertiesOf(schema).filter(
    ([, property]) =>
      Object.hasOwn(property, "const") || property.enum?.length === 1,
  );
  if (constants.length)
    return constants.every(
      ([key, property]) =>
        object[key] === (property.const ?? property.enum?.[0]),
    );
  if (schema.type === "array") return Array.isArray(value);
  if (
    schema.type === "object" &&
    typeof schema.additionalProperties === "object" &&
    Array.isArray(value)
  )
    return true;
  if (schema.type === "object")
    return (
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      (schema.required ?? []).every((key) => Object.hasOwn(object, key))
    );
  return schema.type === typeof value;
}
export function structuralValidator(module: InstrumentModule) {
  return z.record(z.string(), z.unknown()).transform((value, context) => {
    try {
      return module.codec.validator.parse(
        workingValue(
          module.schema,
          { schema: module.schema, pointer: "#" },
          value,
        ),
      );
    } catch (error) {
      if (error instanceof z.ZodError)
        for (const issue of error.issues)
          context.addIssue({
            ...issue,
            path:
              issue.code === "unrecognized_keys"
                ? []
                : (issuePathToFieldPath(issue.path, value) ?? []),
          });
      else context.addIssue({ code: "custom", message: String(error) });
      return z.NEVER;
    }
  });
}
