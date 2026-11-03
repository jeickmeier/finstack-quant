"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import {
  FinstackChart,
  heatmap,
} from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import type { HeatmapColors } from "@/components/finstack/shared/chart/finstack-chart/heatmap";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** analytics.Performance#correlationMatrix as returned, with the panel's ticker names in matrix order. */
export interface CorrelationMatrixData {
  /** Row and column order of the matrix. */
  readonly tickerNames: readonly string[];
  /** Pearson correlations, one row per ticker. */
  readonly matrix: readonly (readonly number[])[];
  /** Rust's flag: true when the raw estimate was projected to the nearest valid correlation matrix. */
  readonly repaired: boolean;
}
export interface CorrelationHeatmapProps {
  /** The native matrix; null while none is available. */
  report: CorrelationMatrixData | null | undefined;
  loading?: boolean;
  /** The native error of the request, shown as returned. */
  error?: string;
  /** Display text of a returned coefficient; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
interface Cell {
  row: string;
  column: string;
  value: number;
}
interface MatrixRow {
  ticker: string;
  values: readonly number[];
}
/** Correlations live in [-1, 1]; zero is the meaningful centre of the ramp. */
const colors: HeatmapColors = { palette: "diverging", domain: [-1, 0, 1] };

/**
 * Draws a native Pearson correlation matrix (analytics.Performance
 * correlationMatrix) as a heatmap and a table, in the returned ticker order,
 * with Rust's repaired stamp: whether Rust projected the estimate to the
 * nearest valid correlation matrix. Every coefficient is drawn as returned.
 */
export function CorrelationHeatmap({
  report,
  loading,
  error,
  formatValue = String,
  density,
}: CorrelationHeatmapProps) {
  const view = useMemo(() => {
    if (!report) return null;
    const names = report.tickerNames;
    const cells: Cell[] = report.matrix.flatMap((row, i) =>
      row.map((value, j) => ({ row: names[i]!, column: names[j]!, value })),
    );
    const columns: ColumnDef<{}, MatrixRow, any>[] = [
      {
        id: "ticker",
        header: "Ticker",
        accessorFn: (row) => row.ticker,
        meta: { className: "text-left" },
      },
      ...names.map((name, j): ColumnDef<{}, MatrixRow, any> => ({
        id: `column-${j}`,
        header: name,
        accessorFn: (row) => row.values[j],
        meta: { className: "text-right" },
        cell: (context) => {
          const value = context.row.original.values[j]!;
          return (
            <span className="finstack-numeric" title={String(value)}>
              {formatValue(value)}
            </span>
          );
        },
      })),
    ];
    // heatmap() throws on a value outside its colour domain.
    const outside = cells.some((cell) => cell.value < -1 || cell.value > 1);
    return {
      rows: report.matrix.map((values, i) => ({ ticker: names[i]!, values })),
      columns,
      preset: outside
        ? null
        : heatmap({
            data: cells,
            x: (cell) => cell.column,
            y: (cell) => cell.row,
            value: (cell) => cell.value,
            key: (cell) => `${cell.row}|${cell.column}`,
            xLabel: "Ticker",
            yLabel: "Ticker",
            valueLabel: "Correlation",
            formatValue,
            ...colors,
          }),
    };
  }, [report, formatValue]);
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!report || !view)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Estimating correlations…" : "No correlation matrix"}
      </p>
    );
  return (
    <section
      aria-label="Return correlation"
      data-density={density}
      className="min-w-0 max-w-full space-y-3 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-sm font-medium">Return correlation</h3>
        {report.repaired ? (
          <span
            role="note"
            aria-label="Repaired"
            className="rounded-sm border border-border bg-muted px-2 py-0.5 text-xs font-semibold uppercase tracking-wide text-foreground"
          >
            Repaired
          </span>
        ) : null}
      </div>
      <p className="text-xs text-muted-foreground">
        {report.repaired
          ? "Rust's pairwise estimate was not a valid correlation matrix and was projected to the nearest valid one (Higham); the coefficients below are the repaired matrix."
          : "Pearson correlations of each pair of tickers over their common active dates, as Rust estimated them; Rust did not need to repair the matrix."}
      </p>
      {view.preset ? (
        <figure className="min-w-0">
          <FinstackChart
            {...view.preset}
            ariaLabel="Correlation heatmap"
            height={360}
          />
        </figure>
      ) : (
        <p role="status" className="text-xs text-muted-foreground">
          A returned coefficient lies outside [-1, 1]; shown in the table only
        </p>
      )}
      <FinstackTable
        caption="Correlation matrix"
        captionVisibility="sr-only"
        data={view.rows}
        columns={view.columns}
        getRowId={(row) => row.ticker}
        emptyState="No tickers returned"
        density={density}
      />
    </section>
  );
}
