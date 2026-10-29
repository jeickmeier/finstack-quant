"use client";
import { useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { DatedCovenantReportsWire } from "@/lib/finstack/generated/types/dated_covenant_reports";
import { Button } from "@/components/ui/button";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  CovenantReport,
  type CovenantReportNumber,
} from "../covenant-report/covenant-report";

type Row = { key: string };
export interface CovenantComplianceProps {
  /** Native DatedCovenantReports rows (statements_analytics.evaluateCovenants or CovenantEngine#evaluateSeries), one column each, in the order supplied. */
  series: readonly DatedCovenantReportsWire[];
  caption?: string;
  captionVisibility?: "visible" | "sr-only";
  /** Display text of a numeric field in the selected date's report. */
  formatValue?: (field: CovenantReportNumber, value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Presents a native covenant series as a covenant × test-date grid of the
 * pass/fail verdicts Rust returned. Covenants are listed in the order they
 * first appear; a covenant with no report on a date (not applicable there)
 * shows a dash. Selecting a test date shows that date's complete reports
 * below; the latest date is selected initially. Nothing is computed.
 */
export function CovenantCompliance({
  series,
  caption = "Covenant compliance",
  captionVisibility,
  formatValue,
  density,
}: CovenantComplianceProps) {
  const [selected, setSelected] = useState<number | null>(null);
  const index = selected ?? series.length - 1;
  const active = series[index];
  const rows: Row[] = [
    ...new Set(series.flatMap((row) => Object.keys(row.reports))),
  ].map((key) => ({ key }));
  const columns: ColumnDef<{}, Row, any>[] = [
    {
      id: "covenant",
      header: "Covenant",
      accessorFn: (row) => row.key,
      meta: { className: "", rowHeader: true },
    },
    ...series.map((dated, column): ColumnDef<{}, Row, any> => ({
      id: `date-${column}`,
      header: () => (
        <Button
          type="button"
          variant={column === index ? "secondary" : "ghost"}
          size="sm"
          aria-pressed={column === index}
          onClick={() => setSelected(column)}
        >
          {dated.as_of}
        </Button>
      ),
      accessorFn: (row) => {
        const report = dated.reports[row.key];
        return report == null ? "—" : report.passed ? "Passed" : "Failed";
      },
      cell: ({ getValue }) =>
        getValue() === "Failed" ? (
          <span className="font-medium text-negative">Failed</span>
        ) : (
          getValue()
        ),
    })),
  ];
  return (
    <section
      aria-label={caption}
      className="space-y-3 font-sans text-sm text-foreground"
    >
      <FinstackTable
        data={rows}
        columns={columns}
        getRowId={(row) => row.key}
        caption={caption}
        captionVisibility={captionVisibility}
        emptyState="No covenant reports returned"
        density={density}
      />
      {active && (
        <CovenantReport
          reports={active.reports}
          caption={`Covenant reports on ${active.as_of}`}
          formatValue={formatValue}
          density={density}
        />
      )}
    </section>
  );
}
