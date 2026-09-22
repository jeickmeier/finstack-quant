/**
 * Map a generated validator's literal path segments to the current editor rows.
 * Zod paths already contain decoded keys: '/' and '~' must never be split or
 * decoded again. Open maps are the editor's {key, value} arrays; their keys never
 * enter TanStack's dot/bracket syntax. Ambiguous or missing rows stay in summary.
 */
export function issuePathToFieldPath(
  path: readonly PropertyKey[],
  working: unknown,
): (string | number)[] | undefined {
  const mapped: (string | number)[] = [];
  let current = working;
  for (const segment of path) {
    if (typeof segment === "symbol") return undefined;
    if (Array.isArray(current) && typeof segment === "string") {
      const rows = current.flatMap((row, index) =>
        row && typeof row === "object" && row.key === segment && "value" in row
          ? [index]
          : [],
      );
      if (rows.length !== 1) return undefined;
      mapped.push(rows[0], "value");
      current = current[rows[0]].value;
    } else {
      if (typeof segment === "string" && /[.\[\]]/.test(segment))
        return undefined;
      mapped.push(segment);
      current =
        current && typeof current === "object"
          ? (current as Record<PropertyKey, unknown>)[segment]
          : undefined;
    }
  }
  return mapped;
}
