import schema from "@/lib/finstack/generated/schemas/market_context_state.json";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
export interface MarketEntry {
  key: string;
  path: readonly (string | number)[];
  label: string;
  value: unknown;
  children: MarketEntry[];
}
/** Walk actual values, adding unavailable root fields from the generated contract only. */
export function marketTree(state: MarketContextStateWire): MarketEntry[] {
  function visit(
    value: unknown,
    path: (string | number)[],
    identity: unknown[],
    label: string,
  ): MarketEntry {
    const entries = Array.isArray(value)
      ? value.map((child, index) => [index, child] as const)
      : value !== null && typeof value === "object"
        ? Object.entries(value)
        : [];
    const candidate = (child: unknown) =>
      child !== null &&
      typeof child === "object" &&
      "id" in child &&
      typeof child.id === "string"
        ? {
            id: child.id,
            ...("type" in child && typeof child.type === "string"
              ? { type: child.type }
              : {}),
          }
        : null;
    const candidates = entries.map(([, child]) => candidate(child));
    const counts = new Map<string, number>();
    for (const id of candidates) {
      const key = JSON.stringify(id);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return {
      key: JSON.stringify(identity),
      path,
      label,
      value,
      children: entries.map(([part, child], index) => {
        const id = candidates[index];
        const segment =
          Array.isArray(value) && id && counts.get(JSON.stringify(id)) === 1
            ? id
            : part;
        return visit(
          child,
          [...path, part],
          [...identity, segment],
          id ? `${part}: ${id.id}` : String(part),
        );
      }),
    };
  }
  const fields = [
    ...new Set([...Object.keys(schema.properties), ...Object.keys(state)]),
  ];
  return fields.map((field) =>
    visit(
      Object.hasOwn(state, field)
        ? state[field as keyof MarketContextStateWire]
        : undefined,
      [field],
      [field],
      field,
    ),
  );
}
/** Every node is addressable; identity uses unambiguous IDs when arrays provide them. */
export function flattenMarket(entries: readonly MarketEntry[]): MarketEntry[] {
  return entries.flatMap((entry) => [entry, ...flattenMarket(entry.children)]);
}
/** JSON pointer display is separate from stable identity, preserving slashes and literal map keys. */
export function marketPointer(path: readonly (string | number)[]) {
  return (
    "/" +
    path
      .map((part) => String(part).replaceAll("~", "~0").replaceAll("/", "~1"))
      .join("/")
  );
}
