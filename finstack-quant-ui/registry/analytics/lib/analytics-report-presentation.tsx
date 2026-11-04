export interface DatedPoint {
  series: string;
  iso: string;
  date: Date;
  value: number;
}
const utc = (iso: string) => new Date(`${iso}T00:00:00Z`);
/** Finite native values against their dates; non-finite entries are omitted, never replaced. */
export function datedPoints(
  series: string,
  dates: readonly string[],
  values: readonly number[],
): DatedPoint[] {
  return dates.flatMap((iso, index) => {
    const value = values[index];
    return typeof value === "number" && Number.isFinite(value)
      ? [{ series, iso, date: utc(iso), value }]
      : [];
  });
}
