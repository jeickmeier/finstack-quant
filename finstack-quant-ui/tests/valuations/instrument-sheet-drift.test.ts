// @vitest-environment node
import { expect, it } from "vitest";
import { instruments } from "../../src/generated/instruments";
import {
  INSTRUMENT_SHEET,
  SPEC_GROUPS,
} from "@/components/finstack/valuations/components/instrument-form/instrument-sheet";
import { promotedTerms } from "@/components/finstack/valuations/components/instrument-form/promoted-term";
import { resolve } from "@/components/finstack/core/components/schema-form/schema-walk";
import {
  propertiesOf,
  type Schema,
} from "@/components/finstack/core/components/schema-form/schema";

/** The linked instrument spec schema that SchemaForm walks, not the raw JSON file. */
async function specOf(type: string) {
  const entry = instruments.find((item) => item.type === type);
  expect(entry, type).toBeDefined();
  const root: Schema = (await entry!.loader()).schema;
  const instrument = resolve(root, {
    schema: root.properties!.instrument!,
    pointer: "#/properties/instrument",
  }).schema;
  const spec = resolve(root, {
    schema: instrument.properties!.spec!,
    pointer: "#",
  }).schema;
  return { root, spec };
}

/** The schema and its nullable or tagged union branches, each with references resolved. */
function branches(root: Schema, schema: Schema): Schema[] {
  const resolved = resolve(root, { schema, pointer: "#" }).schema;
  const union = resolved.anyOf ?? resolved.oneOf;
  return union ? union.flatMap((branch) => branches(root, branch)) : [resolved];
}

/** Follow a dotted path with `[n]` array steps, or undefined when a segment is missing. */
function walk(root: Schema, schema: Schema, path: string): Schema | undefined {
  let current: Schema | undefined = schema;
  for (const segment of path.split(".")) {
    const [, key, index] = /^([^[]+)(\[\d+\])?$/.exec(segment) ?? [];
    current = branches(root, current!)
      .map((branch) => propertiesOf(branch).find(([name]) => name === key))
      .find(Boolean)?.[1];
    if (current && index)
      current = branches(root, current).find((branch) => branch.items)?.items;
    if (!current) return undefined;
  }
  return current;
}

it.each(Object.entries(SPEC_GROUPS))(
  "groups only %s spec keys that exist in the generated schema",
  async (type, groups) => {
    const { spec } = await specOf(type);
    const keys = propertiesOf(spec).map(([key]) => key);
    expect(keys).toEqual(expect.arrayContaining(Object.keys(groups)));
  },
);

it.each(Object.entries(promotedTerms))(
  "promotes a %s path that exists in the generated schema",
  async (type, term) => {
    const { root, spec } = await specOf(type);
    const prefix = `${INSTRUMENT_SHEET.path}.`;
    expect(term.path.startsWith(prefix)).toBe(true);
    expect(walk(root, spec, term.path.slice(prefix.length))).toBeDefined();
  },
);

it("clears only bond market quotes that exist in the generated schema", async () => {
  const { root, spec } = await specOf("bond");
  const quotes = walk(root, spec, "instrument_pricing_overrides.market_quotes");
  expect(quotes).toBeDefined();
  const keys = branches(root, quotes!).flatMap((branch) =>
    propertiesOf(branch).map(([key]) => key),
  );
  expect(promotedTerms.bond.exclusiveWith?.length).toBeGreaterThan(0);
  expect(keys).toEqual(
    expect.arrayContaining([...promotedTerms.bond.exclusiveWith!]),
  );
});
