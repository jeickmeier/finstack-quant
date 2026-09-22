const pointer = (key) => key.replaceAll("~", "~0").replaceAll("/", "~1");

/**
 * Look up a local JSON Pointer against a schema root.
 * Decodes `~1` before `~0`, returns the raw target with no sibling merge and
 * no URI fetching. `"#"` returns the root; missing or inherited members
 * return `undefined`; nonlocal references throw.
 */
export function schemaAt(root, reference) {
  if (reference === "#") return root;
  if (!reference.startsWith("#/"))
    throw new Error(`Unsupported schema reference: ${reference}`);
  let value = root;
  for (const part of reference.slice(2).split("/")) {
    const key = part.replaceAll("~1", "/").replaceAll("~0", "~");
    if (!value || typeof value !== "object" || !Object.hasOwn(value, key))
      return undefined;
    value = value[key];
  }
  return value;
}

/** Complete signed-integer lexical form: no prefix, whitespace, or exponent. */
export const integerText = /^-?\d+$/;

/**
 * Convert finite numeric edit text while preserving invalid or incomplete
 * input verbatim. `integer` requires a safe integer; otherwise the text must
 * be a finite JSON-number lexical form. No financial bounds are validated and
 * decimal-string fields are unaffected.
 */
export function numericEdit(text, integer = false) {
  const pattern = integer ? integerText : /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/;
  const value = Number(text);
  return pattern.test(text) &&
    (integer ? Number.isSafeInteger(value) : Number.isFinite(value))
    ? value
    : text;
}

export const maps = new Set(["$defs", "properties", "patternProperties"]);
export const arrays = new Set(["allOf", "anyOf", "oneOf", "prefixItems"]);
export const singles = new Set([
  "items",
  "additionalProperties",
  "propertyNames",
  "contains",
  "not",
]);
export const annotations = new Set([
  "title",
  "description",
  "default",
  "examples",
  "$comment",
  "readOnly",
  "writeOnly",
  "deprecated",
]);

/** Visit only schema positions; annotations and example data may contain arbitrary keys. */
export function mapChildren(schema, visit) {
  return Object.fromEntries(
    Object.entries(schema).map(([key, value]) => {
      if (maps.has(key))
        return [
          key,
          Object.fromEntries(
            Object.entries(value).map(([name, child]) => [
              name,
              visit(child, `${key}/${pointer(name)}`),
            ]),
          ),
        ];
      if (arrays.has(key))
        return [key, value.map((child, i) => visit(child, `${key}/${i}`))];
      if (singles.has(key)) return [key, visit(value, key)];
      return [key, value];
    }),
  );
}

/**
 * Mechanical converter projection, leaving the committed source bundle intact.
 * Defaults are annotations, not instructions to mutate or accept missing input.
 * Zod cannot consume unevaluatedProperties; lower only the closed object union
 * used by the canonical contracts. Reject every other shape rather than weaken it.
 */
export function converterSchema(schema, target = "zod") {
  if (typeof schema === "boolean") return schema;
  let body = mapChildren(schema, (child) => converterSchema(child, target));
  delete body.default;
  delete body.examples;
  const assertions = [
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "pattern",
    "format",
    "minItems",
    "maxItems",
    "uniqueItems",
  ];
  if (assertions.some((key) => Object.hasOwn(body, key))) {
    if (!body.type)
      throw new Error("Unsupported assertion without an explicit type");
    if (Object.hasOwn(body, "const") || body.enum)
      throw new Error("Unsupported literal with sibling assertions");
  }
  if (body.unevaluatedProperties !== undefined) {
    const allowed = new Set([
      "type",
      "properties",
      "required",
      "oneOf",
      "unevaluatedProperties",
      ...annotations,
    ]);
    const branchAllowed = new Set([
      "type",
      "properties",
      "required",
      ...annotations,
    ]);
    if (
      body.unevaluatedProperties !== false ||
      body.type !== "object" ||
      !body.oneOf?.length ||
      Object.keys(body).some((key) => !allowed.has(key)) ||
      body.oneOf.some(
        (branch) =>
          branch.type !== "object" ||
          Object.keys(branch).some((key) => !branchAllowed.has(key)),
      )
    ) {
      throw new Error("Unsupported unevaluatedProperties shape");
    }
    // Closing each branch is equivalent only if branches cannot both match.
    const discriminated = Object.keys(body.oneOf[0].properties ?? {}).some(
      (key) => {
        const values = body.oneOf.map(
          (branch) => branch.properties?.[key]?.const,
        );
        return (
          body.oneOf.every(
            (branch, i) =>
              branch.required?.includes(key) && typeof values[i] === "string",
          ) && new Set(values).size === values.length
        );
      },
    );
    if (!discriminated) throw new Error("Unsupported overlapping closed union");
    const { properties = {}, required = [], oneOf, ...rest } = body;
    delete rest.type;
    delete rest.unevaluatedProperties;
    body = {
      ...rest,
      oneOf: oneOf.map((branch) => ({
        ...branch,
        properties: Object.fromEntries(
          [
            ...new Set([
              ...Object.keys(properties),
              ...Object.keys(branch.properties ?? {}),
            ]),
          ].map((key) => [
            key,
            Object.hasOwn(properties, key) &&
            Object.hasOwn(branch.properties ?? {}, key)
              ? { allOf: [properties[key], branch.properties[key]] }
              : (properties[key] ?? branch.properties[key]),
          ]),
        ),
        required: [...new Set([...required, ...(branch.required ?? [])])],
        additionalProperties: false,
      })),
    };
  }
  if (target === "typescript" && body.prefixItems) {
    const { prefixItems, items = true, ...rest } = body;
    body = {
      ...rest,
      items: prefixItems.slice(0, body.maxItems ?? prefixItems.length),
      additionalItems: body.maxItems <= prefixItems.length ? false : items,
    };
  }
  // Zod resolves $ref before its siblings. Intersection preserves sibling assertions.
  if (
    body.$ref &&
    Object.keys(body).some(
      (key) => key !== "$ref" && !annotations.has(key) && !key.startsWith("x-"),
    )
  ) {
    const { $ref, $defs, $id, $schema, ...siblings } = body;
    body = {
      ...($defs ? { $defs } : {}),
      ...($id ? { $id } : {}),
      ...($schema ? { $schema } : {}),
      allOf: [{ $ref }, siblings],
    };
  }
  return body;
}
