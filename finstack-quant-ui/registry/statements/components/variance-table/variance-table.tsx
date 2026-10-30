"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type { ReactNode } from "react";
import type { ScenarioDiffWire } from "@/lib/finstack/generated/types/scenario_diff";
import { formatRate, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** Native statements_analytics VarianceReport, as ScenarioDiff.variance returns it. */
export type VarianceReportWire = ScenarioDiffWire["variance"];
type VarianceRow = VarianceReportWire["rows"][number];
/** Rust documents `pct_var` as a fraction (`abs_var / baseline`); shown as a percent. */
const fraction = {
  source: "statements_analytics::VarianceRow::pct_var (fraction)",
  wire: "decimal",
  display: "percent",
} as const;
/** Every supplied digit, grouped; a non-finite native value shows as itself. */
function exactValue(_metric: string, value: number): string {
  return Number.isFinite(value) ? groupDecimal(String(value)) : String(value);
}
/** Percent text of a native fraction; absent or null means Rust found the baseline effectively zero. */
function percentText(value: number | null | undefined): string {
  if (value == null) return "n/m";
  if (!Number.isFinite(value)) return String(value);
  return `${formatRate(value, fraction).text}%`;
}
export interface VarianceTableProps {
  /** Native VarianceReport; rows are shown in native order, never sorted or totalled. */
  report: VarianceReportWire;
  caption: string;
  captionVisibility?: "visible" | "sr-only";
  /** Display name of a metric id; defaults to `labelFor`. */
  metricLabel?: (metric: string) => string;
  /** Display text of a baseline, comparison or absolute variance value; defaults to every supplied digit. */
  formatValue?: (metric: string, value: number) => string;
  density?: "compact" | "comfortable";
  loading?: boolean;
  error?: string;
  emptyState?: ReactNode;
}
/**
 * Presents a native variance report: one row per (metric, period) in native
 * order with the baseline and comparison values under their native labels, the
 * absolute variance and the fractional variance as a percent ("n/m" when Rust
 * leaves it unset because the baseline is effectively zero). It computes
 * nothing.
 */
export function VarianceTable({
  report,
  caption,
  captionVisibility,
  metricLabel = labelFor,
  formatValue = exactValue,
  density,
  loading,
  error,
  emptyState,
}: VarianceTableProps) {
  const numeric = { className: "text-right finstack-numeric" };
  const columns: ColumnDef<{}, VarianceRow, any>[] = [
    {
      id: "metric",
      header: "Metric",
      accessorFn: (row) => metricLabel(row.metric),
      meta: { className: "", rowHeader: true },
    },
    { id: "period", header: "Period", accessorFn: (row) => row.period },
    {
      id: "baseline",
      header: report.baseline_label,
      accessorFn: (row) => formatValue(row.metric, row.baseline),
      meta: numeric,
    },
    {
      id: "comparison",
      header: report.comparison_label,
      accessorFn: (row) => formatValue(row.metric, row.comparison),
      meta: numeric,
    },
    {
      id: "abs_var",
      header: "Variance",
      accessorFn: (row) => formatValue(row.metric, row.abs_var),
      meta: numeric,
    },
    {
      id: "pct_var",
      header: "Variance %",
      accessorFn: (row) => percentText(row.pct_var),
      meta: numeric,
    },
  ];
  return (
    <FinstackTable
      data={report.rows}
      columns={columns}
      getRowId={(row) => `${row.metric}@${row.period}`}
      caption={caption}
      captionVisibility={captionVisibility}
      density={density}
      loading={loading}
      error={error}
      emptyState={emptyState}
    />
  );
}
