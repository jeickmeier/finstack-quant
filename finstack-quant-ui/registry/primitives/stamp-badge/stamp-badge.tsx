import type { ValuationResultWire } from "@/lib/finstack/generated/types/valuation_result";
/** Displays supplied policy/version stamps without assigning defaults or recalculating values. */
export function StampBadge({
  meta,
}: {
  meta: ValuationResultWire["meta"] | null | undefined;
}) {
  if (!meta)
    return (
      <span className="text-xs text-muted-foreground">
        Metadata unavailable
      </span>
    );
  const values = [
    meta.numeric_mode,
    meta.rounding.mode,
    ...Object.entries(meta.rounding.output_scale_by_currency).map(
      ([currency, scale]) => `${currency}: ${scale} decimals`,
    ),
    meta.fx_policy_applied,
    meta.version,
  ].filter((value) => value != null);
  return (
    <div aria-label="Calculation metadata" className="flex flex-wrap gap-1">
      {values.map((value, index) => (
        <span
          key={`${index}:${value}`}
          className="rounded-sm border border-border px-1 font-mono text-xs text-muted-foreground"
        >
          {value}
        </span>
      ))}
    </div>
  );
}
