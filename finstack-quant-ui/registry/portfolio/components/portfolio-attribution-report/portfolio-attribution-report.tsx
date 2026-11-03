"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  MoneyValue as Money,
  PnlBucketShare,
  PortfolioAttribution,
  ReconciliationReport,
} from "finstack-quant-wasm";
import { Badge } from "@/components/ui/badge";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import {
  PnlAttributionReport,
  PnlFactorBuckets,
} from "@/components/finstack/attribution/components/pnl-attribution-report/pnl-attribution-report";

/** One position's native attribution with the Rust helpers its report shows (usePortfolioAttribution's positions). */
export interface PortfolioAttributionPosition {
  /** Position id, in the order Rust keyed by_position. */
  readonly positionId: string;
  /** The position's PnlAttribution, in its instrument's currency. */
  readonly attribution: PortfolioAttribution["by_position"][string];
  /** Rust explain text of the position (attribution.pnlAttributionExplainText). */
  readonly explain: string;
  /** Rust residual check of the position (attribution.pnlAttributionResidualWithinTolerance). */
  readonly residualWithinTolerance: boolean;
  /** Rust buckets of the position (attribution.pnlAttributionPctOfTotalByBucket). */
  readonly buckets: readonly PnlBucketShare[];
}

/** A native portfolio attribution with the Rust helpers shown beside it, as useAttributePortfolioPnl returns it. */
export interface PortfolioAttributionData {
  /** portfolio.attributePortfolioPnl, as returned. */
  readonly attribution: PortfolioAttribution;
  /** portfolio.portfolioAttributionExplainText. */
  readonly explain: string;
  /** portfolio.portfolioAttributionReconciliationCheck at the host's tolerance. */
  readonly reconciliation: ReconciliationReport;
  /** portfolio.portfolioAttributionPctOfTotalByBucket, in Rust's order. */
  readonly buckets: readonly PnlBucketShare[];
  /** Every by_position entry with its Rust helpers, in by_position order. */
  readonly positions: readonly PortfolioAttributionPosition[];
}

export interface PortfolioAttributionReportProps {
  /** The native attribution and its Rust helpers; shown as returned. */
  report: PortfolioAttributionData | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}

/** A returned position bucket as exact money, else a "not returned" mark. */
function BucketCell({ bucket }: { bucket: PnlBucketShare | undefined }) {
  return bucket ? (
    <MoneyValue value={bucket.amount as Money} />
  ) : (
    <span className="text-muted-foreground">Not returned</span>
  );
}

/** Position rows: id, currency and total P&L, then one column per book bucket, each cell the position's own Rust bucket of that field. */
function positionColumns(
  fields: readonly string[],
): ColumnDef<{}, PortfolioAttributionPosition, any>[] {
  return [
    {
      id: "position",
      header: "Position",
      accessorFn: (row) => row.positionId,
      meta: { className: "text-left font-mono" },
    },
    {
      id: "currency",
      header: "Currency",
      accessorFn: (row) => row.attribution.total_pnl.currency,
      meta: { className: "text-left" },
    },
    {
      id: "total",
      header: "Total P&L",
      accessorFn: (row) => row.attribution.total_pnl.amount,
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => (
        <MoneyValue
          value={context.row.original.attribution.total_pnl as Money}
        />
      ),
    },
    ...fields.map(
      (field): ColumnDef<{}, PortfolioAttributionPosition, any> => ({
        id: field,
        header: labelFor(field),
        accessorFn: (row) =>
          row.buckets.find((bucket) => bucket.field === field)?.amount.amount,
        meta: { className: "text-right finstack-numeric" },
        cell: (context) => (
          <BucketCell
            bucket={context.row.original.buckets.find(
              (bucket) => bucket.field === field,
            )}
          />
        ),
      }),
    ),
  ];
}

/** Rust's reconciliation verdict and figures at the host's tolerance, and the invalid flag. */
function ReconciliationBadge({
  attribution,
  reconciliation,
}: {
  attribution: PortfolioAttribution;
  reconciliation: ReconciliationReport;
}) {
  return (
    <div
      aria-label="Reconciliation"
      className="flex flex-wrap items-center gap-2 text-xs"
    >
      {attribution.result_invalid && (
        <Badge variant="destructive">Result invalid</Badge>
      )}
      {reconciliation.is_reconciled ? (
        <Badge variant="secondary">Reconciled</Badge>
      ) : (
        <Badge variant="destructive">Not reconciled</Badge>
      )}
      <span className="text-muted-foreground">
        Total P&L less its buckets {formatRaw(reconciliation.total_residual)}{" "}
        {attribution.total_pnl.currency} · tolerance{" "}
        {formatRaw(reconciliation.tolerance)} {attribution.total_pnl.currency}
      </span>
    </div>
  );
}

/**
 * Display of a native portfolio P&L attribution: the book's total, Rust's
 * reconciliation at the host's tolerance, the base-currency factor buckets as
 * a table and signed chart, every position's native-currency P&L by the same
 * factors, the selected position's full attribution report, Rust's explain
 * text and the JSON. Nothing is summed, differenced or converted.
 */
export function PortfolioAttributionReport({
  report,
  loading,
  error,
  density = "compact",
}: PortfolioAttributionReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        {loading ? "Attributing portfolio P&L…" : "No portfolio attribution"}
      </p>
    );
  return <ReportView report={report} density={density} />;
}

function ReportView({
  report,
  density,
}: {
  report: PortfolioAttributionData;
  density: "compact" | "comfortable";
}) {
  const { attribution, buckets, positions } = report;
  const currency = attribution.total_pnl.currency;
  const columns = useMemo(
    () => positionColumns(buckets.map((bucket) => bucket.field)),
    [buckets],
  );
  // The first returned position starts selected; the user picks another row.
  const [selected, setSelected] = useState(positions[0]?.positionId);
  const position = positions.find((row) => row.positionId === selected);
  return (
    <section
      aria-label="Portfolio attribution"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="space-y-2">
        <MoneyValue
          value={attribution.total_pnl as Money}
          label="Book P&L"
          prominent
        />
        <p className="text-muted-foreground">
          Book P&amp;L in {currency}, by factor at closing FX. Positions are in
          each instrument&apos;s own currency and exclude FX translation, so
          they do not add up to the book figures.
        </p>
        <ReconciliationBadge
          attribution={attribution}
          reconciliation={report.reconciliation}
        />
      </div>
      <Section title="Book factors">
        <PnlFactorBuckets
          buckets={buckets}
          currency={currency}
          caption="Book P&L by factor"
          density={density}
        />
      </Section>
      <Section title="Positions">
        <FinstackTable
          data={positions as PortfolioAttributionPosition[]}
          columns={columns}
          getRowId={(row) => row.positionId}
          caption="P&L by position and factor in each position's currency; activate a row for its attribution"
          captionVisibility="sr-only"
          density={density}
          emptyState="No positions returned"
          onRowActivate={(row) => setSelected(row.positionId)}
        />
      </Section>
      {position && (
        <section
          aria-label={`Position ${position.positionId}`}
          className="space-y-3"
        >
          <h3 className="text-sm font-medium">
            Position <span className="font-mono">{position.positionId}</span>
          </h3>
          <PnlAttributionReport
            attribution={position.attribution}
            buckets={position.buckets}
            explain={position.explain}
            residualWithinTolerance={position.residualWithinTolerance}
            density={density}
          />
        </section>
      )}
      <Section title="Book explain">
        <pre className="overflow-x-auto rounded-md bg-muted p-3 font-mono text-xs">
          {report.explain}
        </pre>
      </Section>
      <JsonViewer
        label="Portfolio attribution JSON"
        text={JSON.stringify(attribution, null, 2)}
        downloadName="portfolio-attribution.json"
        density={density}
      />
    </section>
  );
}
