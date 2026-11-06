"use client";
import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import type { ArbitrageReportWire } from "@/lib/finstack/generated/types/arbitrage_report";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { useSurfaceAttributes } from "@/components/finstack/shared/primitives/surface/surface";
import {
  useCheckSurface,
  type CheckSurfaceRequest,
} from "@/hooks/models/use-check-surface/use-check-surface";

export interface SurfaceArbitrageBadgeProps {
  /** Canonical stored surface; Rust checks only unshifted Black lognormal quotes on a strike axis and reports any other convention. */
  surface: CheckSurfaceRequest["surface"];
  /** The Rust ArbitrageCheckConfig: host-supplied `forward_prices` in the surface's strike units (one to broadcast or one per expiry) and any optional tolerance or check switches. Omitted or null means no check is requested. */
  config?: CheckSurfaceRequest["config"] | null;
  density?: "compact" | "comfortable";
}

type Violation = {
  index: number;
  violation: ArbitrageReportWire["violations"][number];
};
type Count = { name: string; count: number };

const countColumns = (name: string): ColumnDef<{}, Count, any>[] => [
  { id: "name", header: name, accessorFn: (row) => row.name },
  {
    id: "count",
    header: "Violations",
    accessorFn: (row) => row.count,
    cell: ({ getValue }) => formatRaw(getValue() as number),
    meta: { className: "text-right finstack-numeric" },
  },
];
const typeColumns = countColumns("Type"),
  severityColumns = countColumns(labelFor("severity"));

const numeric = (
  id: string,
  header: string,
  pick: (row: Violation["violation"]) => number | null | undefined,
): ColumnDef<{}, Violation, any> => ({
  id,
  header,
  accessorFn: (row) => pick(row.violation),
  cell: ({ getValue }) => formatRaw(getValue() as number | null | undefined),
  meta: { className: "text-right finstack-numeric" },
});

const violationColumns: ColumnDef<{}, Violation, any>[] = [
  {
    id: "violation_type",
    header: labelFor("violation_type"),
    accessorFn: (row) => labelFor(row.violation.violation_type),
  },
  {
    id: "severity",
    header: labelFor("severity"),
    accessorFn: (row) => labelFor(row.violation.severity),
  },
  numeric("expiry", "Expiry (years)", (row) => row.location.expiry),
  numeric(
    "adjacent_expiry",
    "Adjacent expiry (years)",
    (row) => row.location.adjacent_expiry,
  ),
  numeric("strike", labelFor("strike"), (row) => row.location.strike),
  numeric("magnitude", "Magnitude (total variance)", (row) => row.magnitude),
  numeric(
    "suggested_fix",
    labelFor("suggested_fix"),
    (row) => row.suggested_fix,
  ),
  {
    id: "description",
    header: labelFor("description"),
    accessorFn: (row) => row.violation.description,
  },
];

/** Rows in the order Rust returned the keys; names are typography only. */
function counts(record: Readonly<Record<string, number>>): Count[] {
  return Object.entries(record).map(([name, count]) => ({
    name: labelFor(name),
    count,
  }));
}

function ReportDetails({
  report,
  density,
}: {
  report: ArbitrageReportWire;
  density: "compact" | "comfortable";
}) {
  const byType = useMemo(() => counts(report.counts_by_type), [report]);
  const bySeverity = useMemo(() => counts(report.counts_by_severity), [report]);
  return (
    <div className="space-y-3">
      <FinstackTable
        data={byType}
        columns={typeColumns}
        getRowId={(row) => row.name}
        caption="Violations by type"
        density={density}
        emptyState="No violations by type"
      />
      <FinstackTable
        data={bySeverity}
        columns={severityColumns}
        getRowId={(row) => row.name}
        caption="Violations by severity"
        density={density}
        emptyState="No violations by severity"
      />
      <FinstackTable
        data={report.violations.map((violation, index) => ({
          index,
          violation,
        }))}
        columns={violationColumns}
        getRowId={(row) => String(row.index)}
        caption="Violations, most severe first"
        density={density}
        emptyState="No violations returned"
      />
    </div>
  );
}

/**
 * Static-arbitrage status of a stored volatility surface from the native
 * models.volatility.checkSurface report: passed, violations, not checked (the
 * native error, such as a quote convention Rust rejects) or no check
 * configuration supplied. The popover shows the returned counts and violations unchanged.
 */
export function SurfaceArbitrageBadge({
  surface,
  config,
  density = "compact",
}: SurfaceArbitrageBadgeProps) {
  const supplied = config != null;
  const request = useMemo<CheckSurfaceRequest | null>(
    () => (config == null ? null : { surface, config }),
    [surface, config],
  );
  const query = useCheckSurface(request);
  const attributes = useSurfaceAttributes();
  const report = query.data;
  let status: string,
    variant: "secondary" | "destructive" | "outline",
    detail: string;
  if (!supplied) {
    status = "Forwards not supplied";
    variant = "outline";
    detail =
      "The check needs forward prices from the host; none were supplied, so it was not run.";
  } else if (query.error) {
    status = "Not checked";
    variant = "outline";
    detail = query.error.message;
  } else if (!report) {
    status = "Checking…";
    variant = "outline";
    detail = "Waiting for the native arbitrage report.";
  } else if (report.passed) {
    status = "Passed";
    variant = "secondary";
    detail = report.violations.length
      ? `No violation above negligible; ${report.violations.length} negligible returned.`
      : "No butterfly, calendar-spread or local-volatility density violation.";
  } else {
    status = `${report.violations.length} violation${report.violations.length === 1 ? "" : "s"}`;
    variant = "destructive";
    detail = "Violations as Rust returned them, most severe first.";
  }
  return (
    <div
      className="flex flex-wrap items-center gap-2 font-sans text-sm text-foreground"
      data-arbitrage-status={status}
    >
      <span className="text-xs text-muted-foreground">Static arbitrage</span>
      <Badge variant={variant}>{status}</Badge>
      <Popover>
        <PopoverTrigger render={<Button variant="outline" size="sm" />}>
          Arbitrage details
        </PopoverTrigger>
        <PopoverContent
          {...attributes}
          align="start"
          className="w-[44rem] max-w-[calc(100vw-2rem)]"
        >
          <PopoverHeader>
            <PopoverTitle>Static arbitrage · {surface.id}</PopoverTitle>
            <PopoverDescription
              role={query.error ? "alert" : undefined}
              className={query.error && supplied ? "text-error" : undefined}
            >
              {detail}
            </PopoverDescription>
          </PopoverHeader>
          {supplied && report ? (
            <div className="max-h-[60vh] overflow-auto">
              <ReportDetails report={report} density={density} />
            </div>
          ) : null}
        </PopoverContent>
      </Popover>
    </div>
  );
}
