import { Badge } from "@/components/ui/badge";
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
  compact = false,
}: {
  meta: Record<string, unknown> | null | undefined;
  /** Keep primary policy badges visible and disclose the full stamp; print retains all fields. */
  compact?: boolean;
}) {
  if (!meta)
    return (
      <span className="text-xs text-muted-foreground">
        Metadata unavailable
      </span>
    );
  const rounding = record(meta.rounding);
  const supplied = (key: string) => (key in meta ? meta[key] : undefined);
  const fields = [
    ["Numeric mode", supplied("numeric_mode")],
    ["Rounding", rounding?.mode],
    ["FX policy", supplied("fx_policy_applied")],
    ["Version", supplied("version")],
    ["Timestamp", supplied("timestamp")],
    ["Parallel", supplied("parallel")],
    ...Object.entries(record(rounding?.output_scale_by_currency) ?? {}).map(
      ([currency, scale]) => [`${currency} output decimals`, scale],
    ),
    ...Object.entries(record(rounding?.ingest_scale_by_currency) ?? {}).map(
      ([currency, scale]) => [`${currency} ingest decimals`, scale],
    ),
  ];
  const short: Record<string, string> = {
    "Numeric mode": "mode",
    Rounding: "rounding",
    "FX policy": "fx",
    Version: "v",
  };
  const renderFields = (entries: typeof fields, quiet = false) =>
    entries.map(([label, value]) => (
      <Badge
        key={String(label)}
        variant={quiet ? "secondary" : "outline"}
        className="rounded-md font-normal"
        render={<div />}
      >
        <dt className={quiet ? "inline text-muted-foreground" : "inline"}>
          {quiet
            ? (short[String(label)] ?? String(label))
            : `${String(label)}:`}{" "}
        </dt>
        <dd className="inline finstack-numeric">
          {typeof value === "string" ||
          typeof value === "number" ||
          typeof value === "boolean"
            ? String(value)
            : value === null
              ? "none"
              : "Unavailable"}
        </dd>
      </Badge>
    ));
  if (!compact)
    return (
      <dl aria-label="Calculation metadata" className="flex flex-wrap gap-1">
        {renderFields(fields)}
      </dl>
    );
  const primary = new Set(["Numeric mode", "Rounding", "FX policy", "Version"]);
  return (
    <div>
      <div className="flex flex-wrap items-center gap-2 print:hidden">
        <dl aria-label="Calculation policy" className="flex flex-wrap gap-1">
          {renderFields(
            fields.filter(
              ([label, value]) =>
                primary.has(String(label)) && value !== undefined,
            ),
            true,
          )}
        </dl>
        <details className="text-xs text-muted-foreground">
          <summary className="cursor-pointer focus-visible:outline-2 focus-visible:outline-ring">
            Full metadata
          </summary>
          <dl
            aria-label="Calculation metadata"
            className="mt-2 flex flex-wrap gap-1"
          >
            {renderFields(fields)}
          </dl>
        </details>
      </div>
      <dl
        aria-label="Printed calculation metadata"
        className="hidden flex-wrap gap-1 print:flex"
      >
        {renderFields(fields)}
      </dl>
    </div>
  );
}
