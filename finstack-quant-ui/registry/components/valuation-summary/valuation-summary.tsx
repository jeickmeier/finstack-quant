import type { ValuationResult } from "finstack-quant-wasm";
import { MoneyValue } from "../../primitives/money-value/money-value";
import {
  StampBadge,
  returnedRounding,
} from "../../primitives/stamp-badge/stamp-badge";
/** Plain supplied output; missing metadata is supported without applying policy defaults. */
export type SuppliedValuation = Pick<
  ValuationResult,
  "instrument_id" | "as_of" | "value" | "measures"
> & { meta?: Record<string, unknown> | null };
export interface ValuationSummaryProps {
  result: SuppliedValuation | null | undefined;
  /** Independent supplied result; no differences, currency conversions or ratios are computed. */
  compareTo?: SuppliedValuation | null;
  compact?: boolean;
  density?: "compact" | "comfortable";
  loading?: boolean;
  error?: string;
}
function Summary({
  result,
  label,
  compact,
}: {
  result: SuppliedValuation | null | undefined;
  label: string;
  compact?: boolean;
}) {
  return (
    <section
      aria-label={label}
      className="min-w-0 space-y-3 border-b border-border pb-3 print:break-inside-avoid"
    >
      <h2 className="text-xs font-medium text-muted-foreground">
        {label === "Valuation" ? "Present value" : label}
      </h2>
      {result ? (
        <>
          <dl
            className={
              compact
                ? "flex flex-wrap gap-x-5 gap-y-2"
                : "grid grid-cols-2 gap-x-5 gap-y-2"
            }
          >
            <div className="w-full">
              <dt className="sr-only">Value</dt>
              <dd className="break-words text-2xl font-semibold leading-tight [&>span]:text-2xl">
                <MoneyValue
                  value={result.value}
                  rounding={returnedRounding(result.meta)}
                />
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">Instrument</dt>
              <dd className="break-all font-mono text-xs">
                {result.instrument_id}
              </dd>
            </div>
            <div>
              <dt className="text-xs text-muted-foreground">As of</dt>
              <dd className="finstack-numeric text-xs">{result.as_of}</dd>
            </div>
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
  compact,
  density,
  loading,
  error,
}: ValuationSummaryProps) {
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
        <Summary result={result} label="Valuation" compact={compact} />
        {compareTo !== undefined && (
          <Summary
            result={compareTo}
            label="Comparison valuation"
            compact={compact}
          />
        )}
      </div>
    </div>
  );
}
