import type { RoundingStamp } from "@/lib/finstack/format/format";
const record = (value: unknown): Record<string, unknown> | undefined =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
/** Read an available returned rounding stamp; partial metadata never supplies an implicit scale. */
export function returnedRounding(
  meta: Record<string, unknown> | null | undefined,
): RoundingStamp | undefined {
  const rounding = record(meta?.rounding);
  const output = record(rounding?.output_scale_by_currency);
  if (
    !rounding ||
    !output ||
    typeof rounding.mode !== "string" ||
    !["bankers", "away_from_zero", "toward_zero", "floor", "ceil"].includes(
      rounding.mode,
    ) ||
    !Object.values(output).every(
      (value) =>
        typeof value === "number" && Number.isInteger(value) && value >= 0,
    )
  )
    return undefined;
  return {
    mode: rounding.mode as RoundingStamp["mode"],
    output_scale_by_currency: output as Record<string, number>,
  };
}
/** Display actual supplied policy/version fields; absent metadata stays explicitly unavailable. */
export function StampBadge({
  meta,
}: {
  meta: Record<string, unknown> | null | undefined;
}) {
  if (!meta)
    return (
      <span className="text-xs text-muted-foreground">
        Metadata unavailable
      </span>
    );
  const rounding = record(meta.rounding);
  const fields = [
    ["Numeric mode", meta.numeric_mode],
    ["Rounding", rounding?.mode],
    ["FX policy", meta.fx_policy_applied],
    ["Version", meta.version],
    ["Timestamp", meta.timestamp],
    ["Parallel", meta.parallel],
    ...Object.entries(record(rounding?.output_scale_by_currency) ?? {}).map(
      ([currency, scale]) => [`${currency} output decimals`, scale],
    ),
    ...Object.entries(record(rounding?.ingest_scale_by_currency) ?? {}).map(
      ([currency, scale]) => [`${currency} ingest decimals`, scale],
    ),
  ];
  return (
    <dl aria-label="Calculation metadata" className="flex flex-wrap gap-1">
      {fields.map(([label, value]) => (
        <div
          key={String(label)}
          className="rounded-sm border border-border px-1 font-mono text-xs text-muted-foreground"
        >
          <dt className="inline">{String(label)}: </dt>
          <dd className="inline">
            {typeof value === "string" ||
            typeof value === "number" ||
            typeof value === "boolean"
              ? String(value)
              : "Unavailable"}
          </dd>
        </div>
      ))}
    </dl>
  );
}
