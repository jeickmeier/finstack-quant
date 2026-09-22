import type { ValuationResult } from "finstack-quant-wasm";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { StampBadge } from "../../primitives/stamp-badge/stamp-badge";
/** Plain supplied output; missing metadata is supported without applying policy defaults. */
export type SuppliedValuation = Pick<
  ValuationResult,
  "instrument_id" | "as_of" | "value" | "measures"
> & { meta?: Record<string, unknown> | null };
export interface ValuationSummaryProps {
  result: SuppliedValuation | null | undefined;
  /** Independent supplied result; no differences, currency conversions or ratios are computed. */
  compareTo?: SuppliedValuation | null;
  /** Prepared display text for `result.value`; the raw exact amount is shown when omitted. */
  formattedValue?: string;
  /** Prepared display text for `compareTo.value`; independent from `formattedValue`. */
  comparisonFormattedValue?: string;
  compact?: boolean;
  /** Model from the completed pricing request; omitted when context is not supplied. */
  model?: string;
  /** Model belonging only to the independently supplied comparison request. */
  comparisonModel?: string;
  density?: "compact" | "comfortable";
  loading?: boolean;
  error?: string;
}
function Summary({
  result,
  label,
  compact,
  model,
  displayText,
}: {
  result: SuppliedValuation | null | undefined;
  label: string;
  compact?: boolean;
  model?: string;
  displayText?: string;
}) {
  return (
    <section
      aria-label={label}
      className="finstack-valuation print:break-inside-avoid"
      data-compact={compact || undefined}
    >
      <h2 className="text-xs font-medium text-muted-foreground">
        {label === "Valuation" ? "Present value" : label}
      </h2>
      {result ? (
        <>
          <div className="finstack-valuation__value">
            <MoneyValue
              value={result.value}
              displayText={displayText}
              prominent
            />
          </div>
          <dl className="finstack-valuation__context">
            <div className="flex flex-col gap-0.5">
              <dt>Instrument</dt>
              <dd className="font-mono text-foreground">
                {result.instrument_id}
              </dd>
            </div>
            <div className="flex flex-col gap-0.5">
              <dt>Valuation date</dt>
              <dd className="font-mono text-foreground">{result.as_of}</dd>
            </div>
            {model !== undefined && (
              <div className="flex flex-col gap-0.5">
                <dt>Model</dt>
                <dd className="font-mono text-foreground">{model}</dd>
              </div>
            )}
          </dl>
          <StampBadge meta={result.meta} compact />
        </>
      ) : (
        <p className="text-sm text-muted-foreground">No valuation supplied</p>
      )}
    </section>
  );
}
/** Value and calculation context for each supplied result, with optional side-by-side comparison. */
export function ValuationSummary({
  result,
  compareTo,
  formattedValue,
  comparisonFormattedValue,
  compact,
  density,
  model,
  comparisonModel,
  loading,
  error,
}: ValuationSummaryProps) {
  const differentCurrencies =
    result && compareTo && result.value.currency !== compareTo.value.currency;
  const differentDates =
    result && compareTo && result.as_of !== compareTo.as_of;
  const contextDifference = [
    differentCurrencies && "currencies",
    differentDates && "valuation dates",
  ]
    .filter(Boolean)
    .join(" and ");
  return (
    <div
      className="space-y-2 font-sans text-foreground"
      data-density={density}
      aria-busy={loading}
    >
      {error && (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      )}
      {loading && (
        <p role="status" className="text-sm text-muted-foreground">
          Loading valuation…
        </p>
      )}
      <div
        className={
          compareTo === undefined ? "grid gap-3" : "grid gap-3 lg:grid-cols-2"
        }
      >
        <Summary
          result={result}
          label="Valuation"
          compact={compact}
          model={model}
          displayText={formattedValue}
        />
        {compareTo !== undefined && (
          <Summary
            result={compareTo}
            label="Comparison valuation"
            compact={compact}
            model={comparisonModel}
            displayText={comparisonFormattedValue}
          />
        )}
      </div>
      {contextDifference && (
        <p
          role="note"
          className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground"
        >
          Different {contextDifference}. Each valuation is shown in its own
          supplied context.
        </p>
      )}
    </div>
  );
}
