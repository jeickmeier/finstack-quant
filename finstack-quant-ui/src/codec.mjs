import { isLosslessNumber, parse, stringify } from "lossless-json";
import { z } from "zod";
import { converterSchema, mapChildren } from "./schema.mjs";

const wide = (schema) =>
  schema &&
  ["int64", "uint64"].includes(schema.format) &&
  (schema.type === "integer" || schema.type?.includes?.("integer"));

function integerSchema(schema) {
  const unsigned = schema.format === "uint64";
  let validator = z
    .bigint()
    .min(unsigned ? 0n : -(1n << 63n))
    .max(unsigned ? (1n << 64n) - 1n : (1n << 63n) - 1n);
  for (const [key, method] of [
    ["minimum", "min"],
    ["maximum", "max"],
    ["exclusiveMinimum", "gt"],
    ["exclusiveMaximum", "lt"],
  ]) {
    if (schema[key] !== undefined) {
      if (!Number.isSafeInteger(schema[key]))
        throw new Error(`Unsafe schema bound: ${key}`);
      validator = validator[method](BigInt(schema[key]));
    }
  }
  if (
    schema.multipleOf !== undefined ||
    schema.const !== undefined ||
    schema.enum !== undefined
  )
    throw new Error("Unsupported wide-integer constraint");
  return validator;
}

function structuralSchema(schema) {
  if (typeof schema === "boolean") return schema;
  for (const key of ["not", "contains", "patternProperties", "propertyNames"]) {
    if (Object.hasOwn(schema, key))
      throw new Error(`Unsupported codec constraint: ${key}`);
  }
  if (wide(schema)) {
    if (
      Array.isArray(schema.type) &&
      (schema.type.length !== 2 || !schema.type.includes("null"))
    )
      throw new Error("Unsupported wide-integer type union");
    integerSchema(schema); // Fail during construction, not on first user input.
    return {
      type: Array.isArray(schema.type) ? ["integer", "null"] : "integer",
    };
  }
  return mapChildren(schema, structuralSchema);
}

// Keep machine-readable path segments beside transport diagnostics. Consumers must
// not reconstruct field semantics from prose or split punctuation-bearing keys.
function wireError(path, message) {
  const pointer = path
    .map((key) => String(key).replaceAll("~", "~0").replaceAll("/", "~1"))
    .join("/");
  return Object.assign(
    new TypeError(`${path.length ? "/" : ""}${pointer}: ${message}`),
    { path },
  );
}

function numeric(value, path) {
  if (isLosslessNumber(value)) {
    const converted = value.valueOf();
    if (typeof converted === "bigint") return converted;
    value = converted;
  }
  if (
    typeof value === "number" &&
    (!Number.isFinite(value) ||
      (Number.isInteger(value) && !Number.isSafeInteger(value)))
  )
    throw wireError(
      path,
      "unsafe numeric value; supply an exact integer token or bigint",
    );
  return value;
}

/** Convert JSON-compatible trees without invoking toJSON or dropping unsupported values. */
function tree(value, leaf, path = [], ancestors = new Set()) {
  if (isLosslessNumber(value) || value === null || typeof value !== "object") {
    if (value === undefined || ["function", "symbol"].includes(typeof value))
      throw wireError(path, "not a JSON value");
    return leaf(value, path);
  }
  if (ancestors.has(value)) throw wireError(path, "cyclic input");
  if (
    !Array.isArray(value) &&
    ![Object.prototype, null].includes(Object.getPrototypeOf(value))
  )
    throw wireError(path, "host object requires an explicit adapter");
  ancestors.add(value);
  const entries = Object.entries(value).map(([key, child]) => [
    key,
    tree(
      child,
      leaf,
      [...path, Array.isArray(value) ? Number(key) : key],
      ancestors,
    ),
  ]);
  const output = Array.isArray(value)
    ? entries.map(([, child]) => child)
    : Object.fromEntries(entries);
  if (
    Array.isArray(value) &&
    (entries.length !== value.length ||
      entries.some(([key], index) => key !== String(index)))
  )
    throw wireError(path, "sparse array");
  ancestors.delete(value);
  return output;
}
const shadow = (value) =>
  tree(value, (item, path) => {
    item = numeric(item, path);
    return typeof item === "bigint" ? 0 : item;
  });

/**
 * Build cached structural validators plus schema-directed numeric conversion.
 * Wide integer bounds come from Rust's integer format and explicit schema bounds.
 * All other domain validation remains in the supplied Rust canonicalizer.
 */
export function createWireCodec(source) {
  const schema = converterSchema(source);
  const structural = structuralSchema(schema);
  // A branch validator still resolves recursive '#' references to the original
  // document root, not to the branch being tested.
  let rootKey = "codecRoot";
  while (Object.hasOwn(structural.$defs ?? {}, rootKey)) rootKey += "_";
  function rebase(node) {
    if (typeof node === "boolean") return node;
    const mapped = mapChildren(node, rebase);
    if (mapped.$ref === "#") mapped.$ref = `#/$defs/${rootKey}`;
    return mapped;
  }
  const { $defs: definitions, ...rootShape } = rebase(structural);
  const branchDefinitions = { ...definitions, [rootKey]: rootShape };
  const validators = new WeakMap();
  const integers = new WeakMap();
  function validatorFor(node) {
    if (typeof node === "boolean") return node ? z.any() : z.never();
    if (!validators.has(node))
      validators.set(
        node,
        z.fromJSONSchema({
          ...rebase(structuralSchema(node)),
          $defs: branchDefinitions,
        }),
      );
    return validators.get(node);
  }
  const rootValidator = z.fromJSONSchema(structural);
  function walk(node, value, host, path) {
    if (typeof node === "boolean") return value;
    if (node.$ref) {
      const target =
        node.$ref === "#"
          ? schema
          : schema.$defs[node.$ref.slice("#/$defs/".length)];
      return walk(target, value, host, path);
    }
    if (value === null) return value;
    if (wide(node)) {
      if (host && typeof value !== "bigint")
        throw wireError(path, "host int64/uint64 must be bigint");
      if (isLosslessNumber(value)) {
        if (!/^-?\d+$/.test(value.value))
          throw wireError(path, "expected an integer token");
        value = BigInt(value.value);
      } else if (typeof value === "number" && Number.isSafeInteger(value))
        value = BigInt(value);
      if (!integers.has(node)) integers.set(node, integerSchema(node));
      try {
        return integers.get(node).parse(value);
      } catch (error) {
        if (error instanceof z.ZodError)
          throw new z.ZodError(
            error.issues.map((issue) => ({
              ...issue,
              path: [...path, ...issue.path],
            })),
          );
        throw error;
      }
    }
    for (const keyword of ["oneOf", "anyOf"]) {
      if (!node[keyword]) continue;
      const matches = [];
      for (const branch of node[keyword]) {
        try {
          const converted = walk(branch, value, host, path);
          const check = validatorFor(branch).safeParse(shadow(converted));
          if (check.success) matches.push(converted);
        } catch (error) {
          if (!(error instanceof z.ZodError || error instanceof TypeError))
            throw error;
        }
      }
      if (!matches.length || (keyword === "oneOf" && matches.length !== 1))
        throw wireError(path, `no unambiguous ${keyword} wire representation`);
      value = matches[0];
    }
    if (node.allOf)
      for (const branch of node.allOf) value = walk(branch, value, host, path);
    if (Array.isArray(value)) {
      const converted = value.map((item, i) =>
        walk(node.prefixItems?.[i] ?? node.items ?? true, item, host, [
          ...path,
          i,
        ]),
      );
      if (node.uniqueItems)
        tree(converted, (item) => {
          if (typeof item === "bigint")
            throw wireError(
              path,
              "unsupported wide-integer uniqueness constraint",
            );
          return item;
        });
      return converted;
    }
    if (value && typeof value === "object" && !isLosslessNumber(value)) {
      return Object.fromEntries(
        Object.entries(value).map(([key, item]) => [
          key,
          walk(
            node.properties?.[key] ?? node.additionalProperties ?? true,
            item,
            host,
            [...path, key],
          ),
        ]),
      );
    }
    value = numeric(value, path);
    if (
      typeof value === "bigint" &&
      (node.type === "number" ||
        node.type === "integer" ||
        Array.isArray(node.type) ||
        Object.hasOwn(node, "const") ||
        node.enum)
    )
      throw wireError(
        path,
        "cannot narrow bigint to the published number representation",
      );
    return value;
  }
  function normalize(value, host) {
    // Check cycles and unsupported host objects before schema recursion.
    value = tree(value, (item) => item);
    value = walk(schema, value, host, []);
    value = tree(value, numeric);
    rootValidator.parse(shadow(value));
    return value;
  }
  const validator = z.unknown().transform((value, context) => {
    try {
      return normalize(value, false);
    } catch (error) {
      if (error instanceof z.ZodError)
        for (const issue of error.issues) context.addIssue(issue);
      else
        context.addIssue({
          code: "custom",
          message: String(error),
          path:
            error instanceof TypeError && Array.isArray(error.path)
              ? error.path
              : [],
        });
      return z.NEVER;
    }
  });
  return {
    validator,
    parse: (text) => validator.parse(parse(text)),
    fromHost: (value) => normalize(value, true),
    stringify(value, canonicalize) {
      const text = stringify(normalize(value, false));
      return canonicalize ? canonicalize(text) : text;
    },
  };
}

/** Lossless JSON preview for plain host data; this does not claim Rust validation. */
export function serializeHost(value) {
  return stringify(tree(value, numeric));
}
