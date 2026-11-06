"use client";
import type { ForecastMetrics } from "finstack-quant-wasm";
import schema from "@/lib/finstack/generated/schemas/forecast_metrics.json";
import { groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";

/** The ForecastMetrics fields, in the generated contract's property order. */
const fields = Object.keys(schema.properties);
/** First paragraph of a field's Rust documentation, from the generated contract. */
function summary(field: string): string {
  const description = (
    schema.properties as Record<string, { description?: string }>
  )[field]?.description;
  return (description?.split("\n\n")[0] ?? "")
    .replaceAll("\n", " ")
    .replaceAll("`", "");
}
/** A returned metric as text: grouped digits, or "n/m" for a null or non-finite value. */
function metricText(value: number | null | undefined): string {
  return typeof value === "number" && Number.isFinite(value)
    ? groupDecimal(String(value))
    : "n/m";
}
export interface ForecastBacktestProps {
  /** Native ForecastMetrics; null while none is available. */
  metrics: ForecastMetrics | null | undefined;
  /** What was backtested, for example "Revenue, 2025Q2 to 2025Q4". */
  caption: string;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * Presents native forecast accuracy metrics (statements_analytics
 * backtestForecast) as returned: one tile per ForecastMetrics field in the
 * contract's order, with the first paragraph of Rust's description. A null or
 * non-finite metric (for example MAPE when every actual is zero) shows as
 * "n/m". Nothing is calculated.
 */
export function ForecastBacktest({
  metrics,
  caption,
  loading,
  error,
  density = "compact",
}: ForecastBacktestProps) {
  return (
    <section
      aria-label={caption}
      data-density={density}
      className="@container min-w-0 space-y-3 font-sans text-sm text-foreground"
    >
      <h4 className="text-sm font-medium">{caption}</h4>
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !metrics ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Backtesting the forecast…" : "No backtest available."}
        </p>
      ) : (
        <div className="grid grid-cols-1 gap-2 @md:grid-cols-3">
          {fields.map((field) => (
            <section
              key={field}
              aria-label={labelFor(field)}
              className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
            >
              <h5 className="text-xs font-medium text-muted-foreground">
                {labelFor(field)}
              </h5>
              <p className="finstack-numeric text-base">
                {metricText(
                  (metrics as Record<string, number | null | undefined>)[field],
                )}
              </p>
              <p className="text-xs text-muted-foreground">{summary(field)}</p>
            </section>
          ))}
        </div>
      )}
    </section>
  );
}
