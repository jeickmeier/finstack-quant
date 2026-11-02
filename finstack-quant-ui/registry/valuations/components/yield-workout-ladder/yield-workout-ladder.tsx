"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { MetricMetadata, valuations } from "finstack-quant-wasm";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

type Ladder = ReturnType<typeof valuations.instruments.bondWorkoutLadder>;
type Row = Ladder["rows"][number];
type Money = Row["redemption_amount"];
/** Metric ids whose units `metricMetadata` declares for the yield and spread columns. */
export const workoutLadderMetrics = ["ytw", "i_spread"] as const;
export interface YieldWorkoutLadderProps {
  /** The native bondWorkoutLadder result; null while none is available. */
  ladder: Ladder | null;
  /** Native metricMetadata of `workoutLadderMetrics`: the yield and spread units. */
  metadata?: readonly MetricMetadata[];
  loading?: boolean;
  /** The native error of the request (no quote on a callable bond, a missing curve), shown as returned. */
  error?: string;
  /** Display text of a returned yield or spread; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
const money = (value: Money) =>
  `${value.currency} ${groupDecimal(value.amount)}`;
/**
 * Yield and I-spread to every call, put and maturity exit of a bond, in Rust's
 * date order, with the row Rust marks as the worst workout. Nothing is compared,
 * sorted or derived here.
 */
export function YieldWorkoutLadder({
  ladder,
  metadata,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: YieldWorkoutLadderProps) {
  const columns = useMemo<ColumnDef<{}, Row, any>[]>(() => {
    const unit = (metric: string) => {
      const declared = metadata?.find((entry) => entry.key === metric)?.unit;
      return declared ? ` (${declared})` : "";
    };
    const rate = (value: number | null | undefined) =>
      value == null ? (
        <span
          className="finstack-na"
          title="This exit has no finite yield: it is on the quote date, or same-day cash covers the price"
        >
          No yield
        </span>
      ) : (
        <span title={String(value)}>{formatValue(value)}</span>
      );
    return [
      {
        id: "date",
        header: labelFor("date"),
        accessorFn: (row) => row,
        cell: ({ getValue }) => {
          const row = getValue() as Row;
          return row.is_worst ? (
            <strong className="font-semibold">{row.date}</strong>
          ) : (
            row.date
          );
        },
        meta: { className: "", rowHeader: true },
      },
      {
        id: "kind",
        header: labelFor("kind"),
        accessorFn: (row) => row.kind,
        cell: ({ getValue }) => labelFor(getValue() as string),
        meta: { className: "" },
      },
      {
        id: "redemption_price_pct",
        header: labelFor("redemption_price_pct"),
        accessorFn: (row) => row,
        cell: ({ getValue }) => {
          const row = getValue() as Row;
          return row.make_whole ? (
            <span title="Make-whole price off the reference curve">
              {formatRaw(row.redemption_price_pct)} make-whole
            </span>
          ) : (
            formatRaw(row.redemption_price_pct)
          );
        },
        meta: { className: "text-right finstack-numeric" },
      },
      {
        id: "redemption_amount",
        header: labelFor("redemption_amount"),
        accessorFn: (row) => row.redemption_amount,
        cell: ({ getValue }) => money(getValue() as Money),
        meta: { className: "text-right finstack-numeric" },
      },
      {
        id: "yield_rate",
        header: `Yield${unit("ytw")}`,
        accessorFn: (row) => row.yield_rate,
        cell: ({ getValue }) => rate(getValue() as number | null | undefined),
        meta: { className: "text-right finstack-numeric" },
      },
      {
        id: "i_spread",
        header: `${labelFor("i_spread")}${unit("i_spread")}`,
        accessorFn: (row) => row.i_spread,
        cell: ({ getValue }) => rate(getValue() as number | null | undefined),
        meta: { className: "text-right finstack-numeric" },
      },
      {
        id: "is_worst",
        header: "Workout",
        accessorFn: (row) => row.is_worst,
        cell: ({ getValue }) =>
          getValue() ? (
            <span className="font-semibold text-error" data-worst="true">
              Worst
            </span>
          ) : null,
        meta: { className: "" },
      },
    ];
  }, [metadata, formatValue]);
  return (
    <section
      aria-label="Yield workout ladder"
      data-density={density}
      className="min-w-0 max-w-full space-y-3 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status" className="text-muted-foreground">
          Solving exits…
        </p>
      ) : !ladder ? (
        <p role="status" className="text-muted-foreground">
          No workout ladder
        </p>
      ) : (
        <>
          <dl className="flex flex-wrap gap-x-6 gap-y-1">
            {(
              [
                ["instrument_id", ladder.instrument_id],
                ["quote_date", ladder.quote_date],
                ["clean_price_pct", formatRaw(ladder.clean_price_pct)],
                ["dirty_price", money(ladder.dirty_price)],
              ] as const
            ).map(([field, value]) => (
              <div key={field} className="flex gap-2">
                <dt className="text-muted-foreground">{labelFor(field)}</dt>
                <dd className="finstack-numeric">{value}</dd>
              </div>
            ))}
          </dl>
          <FinstackTable
            data={ladder.rows}
            columns={columns}
            // Rust keeps a row per candidate, so two can share a date and kind.
            getRowId={(row) => String(ladder.rows.indexOf(row))}
            caption="Yield to each exit"
            density={density}
            emptyState="Rust returned no exits"
          />
          <p className="text-xs text-muted-foreground">
            Every exit is solved at this one price. The worst row is the one the
            ytw metric selects, marked by Rust.
          </p>
        </>
      )}
    </section>
  );
}
