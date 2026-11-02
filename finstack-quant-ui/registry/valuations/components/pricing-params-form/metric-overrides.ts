import bondSchema from "@/lib/finstack/generated/schemas/bond.json";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import type {
  InstrumentModule,
  Schema,
} from "@/components/finstack/core/components/schema-form/schema";
function reachableDefs(
  node: Schema,
  defs: Record<string, Schema>,
  used: Record<string, Schema> = {},
): Record<string, Schema> {
  const visit = (value: unknown) => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) {
      for (const item of value) visit(item);
      return;
    }
    const record = value as Schema;
    const ref = record.$ref;
    if (ref?.startsWith("#/$defs/")) {
      const name = ref.slice("#/$defs/".length);
      if (!Object.hasOwn(used, name)) {
        const target = defs[name];
        if (!target) throw new Error(`Missing schema reference: ${ref}`);
        used[name] = target;
        visit(target);
      }
    }
    for (const child of Object.values(record)) visit(child);
  };
  visit(node);
  return used;
}
function metricOverridesModule(): InstrumentModule {
  const bond = bondSchema as unknown as Schema;
  const defs = bond.$defs ?? {};
  const source = Object.values(defs).find(
    (node) => node?.title === "Metric Pricing Overrides",
  );
  if (!source) throw new Error("Metric Pricing Overrides schema is missing");
  const schema = structuredClone({
    ...source,
    title: "Pricing overrides",
    $defs: reachableDefs(source, defs),
  });
  return {
    schema,
    metadata: [],
    codec: createWireCodec(schema),
    example: {},
  };
}
/** The MetricPricingOverrides schema from the bond contract; every instrument shares it. */
export const metricOverrides = metricOverridesModule();
/** Host values of an overrides document; text that is not an object starts empty. */
export function overrideValues(
  text: string | null | undefined,
): Record<string, unknown> {
  if (!text) return {};
  try {
    const value = JSON.parse(text) as unknown;
    return value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}
/** The overrides document without blank, empty or null fields; undefined when nothing remains. */
export function compactOverrides(text: string): string | undefined {
  const compact = (value: unknown): unknown => {
    if (typeof value === "string")
      return value.trim() === "" ? undefined : value;
    if (value == null) return undefined;
    if (Array.isArray(value)) {
      const items = value.map(compact);
      return items.some((item) => item !== undefined) ? items : undefined;
    }
    if (typeof value === "object") {
      const entries = Object.entries(value).flatMap(([key, item]) => {
        const next = compact(item);
        return next === undefined ? [] : [[key, next] as const];
      });
      return entries.length ? Object.fromEntries(entries) : undefined;
    }
    return value;
  };
  const value = compact(JSON.parse(text) as unknown);
  return value && typeof value === "object" && !Array.isArray(value)
    ? JSON.stringify(value)
    : undefined;
}
