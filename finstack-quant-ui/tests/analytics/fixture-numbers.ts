/** Restore a numeric result cell from the canonical core nonfinite wire tokens. */
export function fixtureNumber(value: number | string): number {
  if (typeof value === "number") return value;
  if (value === "nan") return Number.NaN;
  if (value === "inf") return Number.POSITIVE_INFINITY;
  if (value === "-inf") return Number.NEGATIVE_INFINITY;
  throw new Error(`Unexpected numeric fixture cell: ${value}`);
}
