"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { MetricMetadata } from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** A native value or the native error of that one id. */
type Outcome =
  { ok: true; value?: number } | { ok: false; error: { message: string } };
/** One metric id and its outcome; the worker's MetricValue. */
export interface LoanYieldRow {
  metric: string;
  value: Outcome;
}
/** The ids the card shows, in display order: yields, spread, cost and the call. */
export const loanYieldMetrics = [
  "ytm",
  "ytw",
  "ytc",
  "yt2y",
  "yt3y",
  "yt4y",
  "discount_margin",
  "all_in_rate",
  "oid_eir_rate",
  "oid_eir_amortization",
  "embedded_option_value",
] as const;
/** The card's ids Rust lists for an instrument type, from metricsForInstrument; the request for useMetricValues. */
export function applicableLoanYieldMetrics(
  available: Readonly<Record<string, readonly string[]>>,
): string[] {
  const listed = Object.values(available).flat();
  return loanYieldMetrics.filter((id) => listed.includes(id));
}
export interface LoanYieldCardProps {
  /** The returned values, each with its own native error; null while none is available. */
  rows: readonly LoanYieldRow[] | null;
  /** Native metricMetadata for the same ids: the label's metric id, unit and group. */
  metadata?: readonly MetricMetadata[];
  loading?: boolean;
  /** The native error of the whole request (a malformed instrument, an invalid market), shown as returned. */
  error?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Yields, discount margin, all-in rate, OID and the call value of a loan, as
 * Rust returned them by metric id. An id Rust rejects shows its native error in
 * its own row and leaves the others. Nothing is classified, summed or derived.
 */
export function LoanYieldCard({
  rows,
  metadata,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: LoanYieldCardProps) {
  const describe = useMemo(() => {
    const entries = new Map(metadata?.map((entry) => [entry.key, entry]));
    return (key: string) => entries.get(key);
  }, [metadata]);
  const columns = useMemo<ColumnDef<{}, LoanYieldRow, any>[]>(
    () => [
      {
        id: "metric",
        header: "Metric",
        accessorFn: (row) => row.metric,
        cell: ({ getValue }) => {
          const metric = getValue() as string;
          return <span title={metric}>{labelFor(metric)}</span>;
        },
        meta: { rowHeader: true },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
        cell: ({ getValue }) => {
          const outcome = getValue() as Outcome;
          if (!outcome.ok)
            return (
              <span
                role="alert"
                className="block min-w-64 whitespace-normal text-xs text-error"
              >
                {outcome.error.message}
              </span>
            );
          const { value } = outcome;
          return value === undefined || !Number.isFinite(value) ? (
            formatRaw(value)
          ) : (
            <span title={String(value)}>{formatValue(value)}</span>
          );
        },
        meta: { className: "text-right finstack-numeric" },
      },
      {
        id: "unit",
        header: "Unit",
        accessorFn: (row) => describe(row.metric)?.unit,
        cell: ({ getValue }) => {
          const unit = getValue() as string | undefined;
          return !unit || unit === "unknown" ? "not declared" : unit;
        },
      },
      {
        id: "group",
        header: "Group",
        accessorFn: (row) => describe(row.metric)?.group,
        cell: ({ getValue }) => formatRaw(getValue() as string | undefined),
      },
    ],
    [describe, formatValue],
  );
  return (
    <section
      aria-label="Loan yields"
      data-density={density}
      className="min-w-0 max-w-full space-y-3 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status" className="text-muted-foreground">
          Pricing metrics…
        </p>
      ) : !rows ? (
        <p role="status" className="text-muted-foreground">
          No metric values
        </p>
      ) : (
        <>
          <FinstackTable
            data={[...rows]}
            columns={columns}
            getRowId={(row) => row.metric}
            caption="Loan yields"
            density={density}
            emptyState="Rust lists none of these metrics for this instrument"
          />
          <p className="text-xs text-muted-foreground">
            Values as Rust returned them. Yields and discount margin solve to a
            quoted clean price when the instrument carries one, otherwise to the
            model price. The call value is callable less straight at one spread,
            so it is negative when the borrower&apos;s call costs the lender.
          </p>
        </>
      )}
    </section>
  );
}
