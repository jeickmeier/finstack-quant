import { describe, expect, it } from "vitest";
import { createWireCodec } from "../src/codec.mjs";

const schema = {
  type: "object",
  properties: {
    counter: { type: "integer", format: "uint64", minimum: 0 },
    signed: { type: ["integer", "null"], format: "int64" },
    amount: { type: "string" },
    n: { type: "integer", format: "uint" },
  },
  required: ["counter", "amount", "n"],
  additionalProperties: false,
};
const codec = createWireCodec(schema);

it("preserves full-width integer tokens and decimal strings using schema formats", () => {
  const text =
    '{"counter":18446744073709551615,"signed":-9223372036854775808,"amount":"100.000000000000000001","n":2}';
  const value = codec.parse(text);
  expect(value).toEqual({
    counter: (1n << 64n) - 1n,
    signed: -(1n << 63n),
    amount: "100.000000000000000001",
    n: 2,
  });
  expect(codec.stringify(structuredClone(value))).toBe(text);
  expect(
    codec.parse('{"counter":42,"signed":null,"amount":"0","n":0}').counter,
  ).toBe(42n);
});

it.each(["18446744073709551616", "-1", '"18446744073709551615"', "1.5", "1e1"])(
  "rejects invalid uint64 token %s",
  (value) =>
    expect(() =>
      codec.parse(`{"counter":${value},"amount":"0","n":1}`),
    ).toThrow(),
);
it("requires bigint in host state and rejects rounded numbers before canonicalization", () => {
  const canonicalize = () => {
    throw new Error("must not be called");
  };
  const value = { counter: 42n, amount: "1", n: 1 };
  expect(codec.fromHost(value)).toEqual(value);
  expect(() => codec.fromHost({ ...value, counter: 42 })).toThrow();
  expect(() =>
    codec.stringify(
      { ...value, counter: Number((1n << 64n) - 1n) },
      canonicalize,
    ),
  ).toThrow();
  expect(() => codec.fromHost({ ...value, n: 1n })).toThrow();
});
it("preserves negative and explicit nonzero wide-integer schema bounds", () => {
  const bounded = createWireCodec({
    type: "integer",
    format: "int64",
    minimum: -5,
    exclusiveMaximum: 9,
  });
  expect(bounded.parse("-5")).toBe(-5n);
  expect(bounded.parse("8")).toBe(8n);
  expect(() => bounded.parse("-6")).toThrow();
  expect(() => bounded.parse("9")).toThrow();
  expect(() =>
    createWireCodec({ type: "integer", format: "int64", maximum: 1e20 }),
  ).toThrow("Unsafe");
});
it("handles wide integers through recursive refs, tuples, nullable and non-type unions", () => {
  const nested = createWireCodec({
    $defs: {
      Node: {
        type: "object",
        properties: {
          mode: { const: "wide" },
          value: { type: "integer", format: "uint64" },
          next: { anyOf: [{ $ref: "#/$defs/Node" }, { type: "null" }] },
        },
        required: ["mode", "value"],
      },
    },
    oneOf: [
      { $ref: "#/$defs/Node" },
      {
        type: "array",
        prefixItems: [{ type: "integer", format: "int64" }, { type: "string" }],
        minItems: 2,
        maxItems: 2,
      },
    ],
  });
  expect(
    nested.parse(
      '{"mode":"wide","value":9007199254740993,"next":{"mode":"wide","value":3}}',
    ),
  ).toEqual({
    mode: "wide",
    value: 9007199254740993n,
    next: { mode: "wide", value: 3n },
  });
  expect(nested.parse('[-9223372036854775808,"x"]')).toEqual([
    -(1n << 63n),
    "x",
  ]);
  expect(() => nested.parse('{"mode":"wide","value":"3"}')).toThrow();
});
it("retains structural failures and rejects non-JSON or lossy trees", () => {
  expect(() => codec.parse('{"counter":1,"n":2}')).toThrow();
  expect(() =>
    codec.parse('{"counter":1,"n":2,"amount":"x","extra":1}'),
  ).toThrow();
  expect(() => codec.parse('{"counter":1,"n":2,"amount":"x","n":3}')).toThrow();
  const any = createWireCodec({});
  for (const value of [
    NaN,
    Infinity,
    undefined,
    new Date(),
    new Float64Array([1]),
    [undefined],
    Array(2),
  ])
    expect(() => any.stringify(value)).toThrow();
  const cyclic = {};
  cyclic.self = cyclic;
  expect(() => any.stringify(cyclic)).toThrow("cyclic");
});

it("fails closed for unsupported wide-integer constraint combinations", () => {
  expect(() =>
    createWireCodec({ type: ["integer", "string"], format: "uint64" }),
  ).toThrow("Unsupported");
  expect(() =>
    createWireCodec({ not: { type: "integer", format: "uint64", minimum: 1 } }),
  ).toThrow("Unsupported");
  const unique = createWireCodec({
    type: "array",
    items: { type: "integer", format: "uint64" },
    uniqueItems: true,
  });
  expect(() => unique.parse("[1,2]")).toThrow(
    "unsupported wide-integer uniqueness",
  );
});
it("resolves root recursion from a selected union branch", () => {
  const recursive = createWireCodec({
    type: "object",
    properties: {
      value: { type: "integer", format: "uint64" },
      child: { anyOf: [{ $ref: "#" }, { type: "null" }] },
    },
    required: ["value"],
  });
  expect(recursive.parse('{"value":1,"child":{"value":2}}')).toEqual({
    value: 1n,
    child: { value: 2n },
  });
});

it("rejects array properties and bigint values outside the declared numeric adapter", () => {
  const sparse = Array(1);
  sparse.extra = "lost";
  expect(() => createWireCodec({}).stringify(sparse)).toThrow();
  expect(() =>
    createWireCodec({ const: 0 }).stringify(18446744073709551615n),
  ).toThrow();
});
