import { describe, expect, it } from "vitest";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { z } from "zod";
import Ajv from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import { compile } from "json-schema-to-typescript";
import { bundleRoot, readContracts } from "../scripts/schema.mjs";
import { discoverFixtures, repoRoot, writeGenerated } from "../scripts/gen.mjs";
import {
  converterSchema,
  integerText,
  numericEdit,
  schemaAt,
} from "../src/schema.mjs";

const uri = (name) => `https://example.test/${name}.json`;
const contracts = (entries) =>
  new Map(
    Object.entries(entries).map(([name, schema]) => [
      uri(name),
      { schema: { $id: uri(name), ...schema } },
    ]),
  );
const oracle = new Ajv({ strict: false, allErrors: true });
addFormats(oracle);
const cases = [
  [
    "numeric bounds",
    { type: "number", exclusiveMinimum: 0, maximum: 2 },
    [1, 2],
    [0, 3, "1"],
  ],
  [
    "required defaults",
    {
      type: "object",
      properties: { n: { type: "integer", default: 2 } },
      required: ["n"],
    },
    [{ n: 3 }],
    [{}, { n: 1.5 }],
  ],
  [
    "optional defaults",
    { type: "object", properties: { n: { type: "integer", default: 2 } } },
    [{}, { n: 3 }],
    [{ n: null }],
  ],
  [
    "additional properties",
    {
      type: "object",
      properties: { n: { type: "number" } },
      additionalProperties: false,
    },
    [{}, { n: 1 }],
    [{ other: 2 }],
  ],
  [
    "open properties",
    { type: "object", properties: { n: { type: "number" } } },
    [{ other: 2 }],
    [{ n: "bad" }],
  ],
  [
    "typed additional properties",
    { type: "object", additionalProperties: { type: "integer" } },
    [{ x: 1 }],
    [{ x: "1" }],
  ],
  [
    "nullable refs",
    {
      $defs: { N: { type: "integer" } },
      anyOf: [{ $ref: "#/$defs/N" }, { type: "null" }],
    },
    [null, 1],
    [1.2, "1"],
  ],
  [
    "reference siblings",
    {
      $defs: { N: { type: "number" } },
      $ref: "#/$defs/N",
      type: "number",
      minimum: 1,
    },
    [1, 2],
    [0, "1"],
  ],
  [
    "exclusive unions",
    { oneOf: [{ type: "number" }, { type: "integer" }] },
    [1.5],
    [1, "x"],
  ],
  [
    "nested non-type tags",
    {
      type: "object",
      properties: {
        choice: {
          oneOf: [
            {
              type: "object",
              properties: { kind: { const: "a" }, value: { type: "number" } },
              required: ["kind", "value"],
              additionalProperties: false,
            },
            {
              type: "object",
              properties: { mode: { const: "b" }, value: { type: "string" } },
              required: ["mode", "value"],
              additionalProperties: false,
            },
          ],
        },
      },
      required: ["choice"],
    },
    [
      { choice: { kind: "a", value: 1 } },
      { choice: { mode: "b", value: "x" } },
    ],
    [{ choice: { mode: "a", value: 1 } }],
  ],
  [
    "tuple boundaries",
    {
      type: "array",
      prefixItems: [{ type: "string" }, { type: "integer" }],
      minItems: 2,
      maxItems: 2,
    },
    [["x", 1]],
    [[], ["x"], ["x", 1, 2], [1, "x"]],
  ],
  [
    "unique values",
    { type: "array", uniqueItems: true },
    [[{ a: 1 }, { a: 2 }]],
    [
      [
        { a: 1, b: 2 },
        { b: 2, a: 1 },
      ],
    ],
  ],
  [
    "date format",
    { type: "string", format: "date" },
    ["2024-02-29"],
    ["2023-02-29", "bad"],
  ],
  [
    "recursive references",
    {
      type: "object",
      properties: { n: { type: "integer" }, child: { $ref: "#" } },
      required: ["n"],
    },
    [{ n: 1, child: { n: 2 } }],
    [{ n: 1, child: { n: "x" } }],
  ],
  [
    "closed flattened union",
    {
      type: "object",
      properties: { id: { type: "string" } },
      required: ["id"],
      unevaluatedProperties: false,
      oneOf: [
        {
          type: "object",
          properties: { kind: { const: "a" }, n: { type: "number" } },
          required: ["kind", "n"],
        },
        {
          type: "object",
          properties: { kind: { const: "b" }, s: { type: "string" } },
          required: ["kind", "s"],
        },
      ],
    },
    [
      { id: "x", kind: "a", n: 1 },
      { id: "x", kind: "b", s: "y" },
    ],
    [
      { id: "x", kind: "a", n: 1, s: "y" },
      { kind: "a", n: 1 },
      { id: "x", kind: "bad" },
    ],
  ],
];

describe("converter construct corpus against JSON Schema 2020-12", () => {
  for (const [name, schema, accepted, rejected] of cases) {
    it(name, () => {
      const validate = oracle.compile(schema);
      const validator = z.fromJSONSchema(converterSchema(schema));
      for (const [values, expected] of [
        [accepted, true],
        [rejected, false],
      ]) {
        for (const value of values) {
          expect(validate(value), JSON.stringify(value)).toBe(expected);
          expect(
            validator.safeParse(value).success,
            JSON.stringify(value),
          ).toBe(expected);
          if (expected) expect(validator.parse(value)).toEqual(value);
        }
      }
    });
  }
  it("rejects unsupported closed-union shapes instead of dropping their constraints", () => {
    expect(() =>
      converterSchema({ type: "object", unevaluatedProperties: false }),
    ).toThrow("Unsupported");
    expect(() =>
      converterSchema({
        type: "object",
        unevaluatedProperties: false,
        oneOf: [
          { type: "object", properties: { a: { type: "string" } } },
          {
            type: "object",
            properties: { a: { type: "string" }, b: { type: "string" } },
          },
        ],
      }),
    ).toThrow("overlapping");
  });
  it("preserves tuple element types in emitted wire declarations", async () => {
    const schema = {
      type: "array",
      prefixItems: [{ type: "string" }, { type: "integer" }],
      minItems: 2,
      maxItems: 2,
    };
    const output = await compile(converterSchema(schema, "typescript"), "Pair");
    expect(output).toContain("[string, number]");
  });
});

describe("offline reachable bundling", () => {
  it("isolates unused definitions, preserves cycles, identities and ref annotations", async () => {
    const sources = contracts({
      root: {
        type: "object",
        properties: {
          a: {
            $ref: `${uri("a")}#/$defs/Same`,
            description: "Field annotation",
          },
          b: { $ref: `${uri("b")}#/$defs/Same` },
        },
        $defs: { Unused: { type: "string", title: "Do not load" } },
      },
      a: {
        $defs: {
          Same: {
            type: "object",
            properties: {
              n: { type: "number" },
              next: { $ref: "#/$defs/Same" },
            },
          },
        },
      },
      b: { $defs: { Same: { type: "string" } } },
    });
    const first = await bundleRoot(uri("root"), sources);
    expect(first).toEqual(await bundleRoot(uri("root"), sources));
    expect(Object.keys(first.schema.$defs)).toHaveLength(2);
    expect(JSON.stringify(first.schema)).not.toContain("Do not load");
    expect(first.schema.properties.a.$ref).not.toBe(
      first.schema.properties.b.$ref,
    );
    expect(
      first.metadata.find((entry) => entry.path === "#/properties/a"),
    ).toMatchObject({
      ref: `${uri("a")}#/$defs/Same`,
      description: "Field annotation",
    });
    expect(
      z
        .fromJSONSchema(converterSchema(first.schema))
        .safeParse({ a: { n: 1, next: { n: 2 } }, b: "x" }).success,
    ).toBe(true);
  });
  it("rejects external refs absent from indexes and dangling internal pointers", async () => {
    await expect(
      bundleRoot(uri("root"), contracts({ root: { $ref: uri("missing") } })),
    ).rejects.toThrow();
    await expect(
      bundleRoot(uri("root"), contracts({ root: { $ref: "#/$defs/Missing" } })),
    ).rejects.toThrow();
  });
  it("rejects unknown validation keywords and preserves annotation instance data", async () => {
    await expect(
      bundleRoot(
        uri("root"),
        contracts({ root: { type: "string", minLenght: 3 } }),
      ),
    ).rejects.toThrow("Unsupported keyword");
    const { schema } = await bundleRoot(
      uri("root"),
      contracts({
        root: {
          type: "object",
          default: { $ref: "ordinary data" },
          "x-unit": "source annotation",
        },
      }),
    );
    expect(schema.default).toEqual({ $ref: "ordinary data" });
    expect(schema["x-unit"]).toBe("source annotation");
  });
  it("retains non-type discriminator constants from the real contracts as metadata", async () => {
    const sources = await readContracts(repoRoot);
    const root = [...sources.keys()].find((key) =>
      key.endsWith("/calibration.schema.json"),
    );
    const { metadata } = await bundleRoot(root, sources);
    const tagged = metadata.filter((entry) => entry.const !== undefined);
    expect(
      tagged.some((entry) => entry.source.includes("/properties/kind")),
    ).toBe(true);
    expect(
      metadata.some(
        (entry) => entry.source.includes("/properties/method") && entry.ref,
      ),
    ).toBe(true);
  });
});

it("discovers fixture additions/removals and splits calibration outputs", async () => {
  const dir = await mkdtemp(resolve(tmpdir(), "ui-fixtures-"));
  try {
    const instrumentDir = resolve(
      dir,
      "finstack-quant/valuations/tests/instruments/json_examples",
    );
    const calibrationDir = resolve(
      dir,
      "finstack-quant/calibration/examples/market_bootstrap",
    );
    await mkdir(instrumentDir, { recursive: true });
    await mkdir(calibrationDir, { recursive: true });
    const sources = await readContracts(repoRoot);
    expect(await discoverFixtures(dir, sources)).toEqual([]);
    const file = resolve(instrumentDir, "new.json");
    await writeFile(file, '{"instrument":{"type":"bond"}}');
    expect(await discoverFixtures(dir, sources)).toHaveLength(1);
    await rm(file);
    expect(await discoverFixtures(dir, sources)).toEqual([]);
    await writeFile(
      resolve(calibrationDir, "new.json"),
      '{"plan":{},"final_market":{}}',
    );
    const discovered = await discoverFixtures(dir, sources);
    expect(discovered.map((entry) => entry.kind)).toEqual([
      "calibration-input",
      "calibration-output",
    ]);
    expect(discovered[1].pointer).toBe("/final_market");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

it("detects modified, missing and stale generated output without writing in check mode", async () => {
  const dir = await mkdtemp(resolve(tmpdir(), "ui-drift-"));
  try {
    await writeGenerated(new Map([["nested/a.ts", "first\n"]]), dir, false);
    await writeGenerated(new Map([["nested/a.ts", "first\n"]]), dir, true);
    await expect(
      writeGenerated(new Map([["nested/a.ts", "second\n"]]), dir, true),
    ).rejects.toThrow("drift");
    expect(await readFile(resolve(dir, "nested/a.ts"), "utf8")).toBe("first\n");
    await expect(
      writeGenerated(new Map([["missing.ts", "new\n"]]), dir, true),
    ).rejects.toThrow("drift");
    await writeGenerated(new Map([["new.ts", "new\n"]]), dir, false);
    await expect(readFile(resolve(dir, "nested/a.ts"))).rejects.toThrow();
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

it("fails closed on assertion combinations the pinned converter would ignore", () => {
  expect(() => converterSchema({ $ref: "#/$defs/N", minimum: 1 })).toThrow(
    "explicit type",
  );
  expect(() =>
    converterSchema({ type: "number", const: 0, minimum: 1 }),
  ).toThrow("literal");
});

it("schemaAt resolves local pointers with escaping and raw targets", () => {
  const root = {
    $defs: {
      "a/b~": { type: "number" },
      "~1": { type: "string" },
    },
    oneOf: [{ type: "number" }, { type: "string" }],
    properties: { disabled: false },
  };
  const before = JSON.stringify(root);
  expect(schemaAt(root, "#")).toBe(root);
  expect(schemaAt(root, "#/$defs/a~1b~0")).toBe(root.$defs["a/b~"]);
  expect(schemaAt(root, "#/$defs/~01")).toBe(root.$defs["~1"]);
  expect(schemaAt(root, "#/properties/disabled")).toBe(false);
  expect(schemaAt(root, "#/oneOf/0")).toBe(root.oneOf[0]);
  expect(schemaAt(root, "#/properties/missing/nested")).toBeUndefined();
  expect(schemaAt(root, "#/constructor")).toBeUndefined();
  expect(() => schemaAt(root, "https://example.test/x.json")).toThrow(
    "Unsupported schema reference",
  );
  expect(JSON.stringify(root)).toBe(before);
});

it("numericEdit converts only complete finite numeric text", () => {
  for (const text of ["", "-", "1.", "1e", "0x10", " 1 ", "+1", "1e999"]) {
    expect(numericEdit(text)).toBe(text);
    expect(numericEdit(text, true)).toBe(text);
  }
  expect(numericEdit("1.25e2")).toBe(125);
  expect(numericEdit("1.25e2", true)).toBe("1.25e2");
  expect(numericEdit("9007199254740993", true)).toBe("9007199254740993");
  expect(numericEdit("-9007199254740991", true)).toBe(-9007199254740991);
  expect(Object.is(numericEdit("-0", true), -0)).toBe(true);
  expect(integerText.test("0x10")).toBe(false);
  expect(integerText.test("1e3")).toBe(false);
});
