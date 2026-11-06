"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type { CovenantReportWire } from "@/lib/finstack/generated/types/covenant_report";
import { formatExact } from "@/lib/finstack/format/format";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { serializeHost } from "@/lib/finstack/codec.mjs";

/** Native covenant reports keyed by covenant instance key, in engine order. */
export type CovenantReports = Readonly<Record<string, CovenantReportWire>>;
/** Numeric CovenantReport fields, in the metric's own units as Rust returns them. */
export type CovenantReportNumber = "actual_value" | "threshold" | "headroom";
type Row = { key: string; report: CovenantReportWire };
export interface CovenantReportProps {
  /** Native reports (CovenantEngine#evaluate, a dated series row, or a valuation result's covenants). */
  reports: CovenantReports;
  caption?: string;
  captionVisibility?: "visible" | "sr-only";
  /** Display text of a numeric field; defaults to every supplied digit. */
  formatValue?: (field: CovenantReportNumber, value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Presents native covenant reports: one row per report in the order supplied,
 * with Rust's test description, actual value, threshold, headroom, pass/fail
 * verdict and details, followed by the complete reports as JSON. A field Rust
 * leaves unset shows as a dash. No verdict, headroom or threshold is computed.
 */
export function CovenantReport({
  reports,
  caption = "Covenant reports",
  captionVisibility,
  formatValue = (_field, value) => formatExact(value),
  density,
}: CovenantReportProps) {
  const numeric = { className: "text-right finstack-numeric" };
  const number = (field: CovenantReportNumber) => (row: Row) => {
    const value = row.report[field];
    return value == null ? "—" : formatValue(field, value);
  };
  const columns: ColumnDef<{}, Row, any>[] = [
    {
      id: "covenant",
      header: "Covenant",
      accessorFn: (row) => row.key,
      meta: { className: "", rowHeader: true },
    },
    {
      id: "covenant_type",
      header: "Test",
      accessorFn: (row) => row.report.covenant_type,
    },
    {
      id: "actual_value",
      header: "Actual",
      accessorFn: number("actual_value"),
      meta: numeric,
    },
    {
      id: "threshold",
      header: "Threshold",
      accessorFn: number("threshold"),
      meta: numeric,
    },
    {
      id: "headroom",
      header: "Headroom",
      accessorFn: number("headroom"),
      meta: numeric,
    },
    {
      id: "passed",
      header: "Native status",
      accessorFn: (row) => (row.report.passed ? "Passed" : "Failed"),
      cell: ({ row }) =>
        row.original.report.passed ? (
          "Passed"
        ) : (
          <span className="font-medium text-negative">Failed</span>
        ),
    },
    {
      id: "details",
      header: "Details",
      accessorFn: (row) => row.report.details ?? "—",
    },
  ];
  return (
    <section
      aria-label={caption}
      className="space-y-2 font-sans text-sm text-foreground"
    >
      <FinstackTable
        data={Object.entries(reports).map(([key, report]) => ({ key, report }))}
        columns={columns}
        getRowId={(row) => row.key}
        caption={caption}
        captionVisibility={captionVisibility}
        emptyState="No covenant reports returned"
        density={density}
      />
      <details>
        <summary className="text-sm">Covenants</summary>
        <JsonViewer
          label="Covenant reports JSON"
          text={serializeHost(reports)}
          density={density}
        />
      </details>
    </section>
  );
}
