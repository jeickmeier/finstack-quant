"use client";
import { useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  MetricMetadata,
  PortfolioValuation,
  PositionValue,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { ValuationSummary } from "@/components/finstack/valuations/components/valuation-summary/valuation-summary";
import { MeasuresGrid } from "@/components/finstack/valuations/components/measures-grid/measures-grid";

/** The parts of a returned portfolio valuation the positions table presents. */
export interface PositionsTableData {
  /** Position ids in spec order (Portfolio#positionIds); rows follow it. */
  positionIds: readonly string[];
  /** portfolio.valuePortfolio as returned. */
  valuation: PortfolioValuation;
}
export interface PositionsTableProps {
  /** The returned valuation; null while none is available. */
  report: PositionsTableData | null;
  /** Rust metricMetadata for the positions' measure keys; absent keeps keys raw. */
  metadata?: readonly MetricMetadata[];
  /** Position shown first in the detail; omitted is the first position. */
  initialPosition?: string;
  loading?: boolean;
  /** The native error message, shown instead of a table. */
  error?: string;
  density?: "compact" | "comfortable";
}
type PositionRow = { id: string; value: PositionValue | undefined };
const columns: ColumnDef<{}, PositionRow, any>[] = [
  {
    id: "position",
    header: "Position",
    accessorFn: (row) => row.id,
    meta: { className: "text-left font-mono" },
  },
  {
    id: "entity",
    header: "Entity",
    accessorFn: (row) => row.value?.entity_id,
    cell: ({ getValue }) => formatRaw(getValue() as string | undefined),
    meta: { className: "text-left font-mono" },
  },
  {
    id: "instrument",
    header: "Instrument",
    accessorFn: (row) => row.value?.valuation_result?.instrument_id,
    cell: ({ getValue }) => formatRaw(getValue() as string | undefined),
    meta: { className: "text-left font-mono" },
  },
  {
    id: "value_native",
    header: "Native value",
    accessorFn: (row) => row.value?.value_native.amount,
    cell: ({ row }) => <MoneyValue value={row.original.value?.value_native} />,
    meta: { className: "text-right" },
  },
  {
    id: "fx_rate",
    header: "FX rate",
    accessorFn: (row) => row.value?.fx_rate,
    cell: ({ getValue }) => formatRaw(getValue() as number | null | undefined),
    meta: { className: "text-right finstack-numeric" },
  },
  {
    id: "value_base",
    header: "Base value",
    accessorFn: (row) => row.value?.value_base.amount,
    cell: ({ row }) => <MoneyValue value={row.original.value?.value_base} />,
    meta: { className: "text-right" },
  },
  {
    id: "risk_metrics_complete",
    header: "Risk complete",
    accessorFn: (row) => row.value?.risk_metrics_complete,
    cell: ({ getValue }) => {
      const value = getValue() as boolean | undefined;
      return value === undefined ? "—" : value ? "Yes" : "No";
    },
    meta: { className: "text-left" },
  },
  {
    id: "inapplicable_metrics",
    header: "Inapplicable metrics",
    accessorFn: (row) => row.value?.inapplicable_metrics?.join(", ") || "—",
    meta: { className: "text-left font-mono" },
  },
];
/**
 * Presents each position of a native portfolio valuation, in spec order:
 * entity, instrument, native and base values, the FX rate Rust applied, risk
 * completeness and the inapplicable metrics. Activating a row shows that
 * position's returned valuation_result through valuation-summary and
 * measures-grid. Nothing is summed, converted or reordered by value.
 */
export function PositionsTable({
  report,
  loading,
  error,
  ...props
}: PositionsTableProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Valuing positions…
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No portfolio valuation
      </p>
    );
  return <PositionsView report={report} {...props} />;
}
function PositionsView({
  report,
  metadata,
  initialPosition,
  density = "compact",
}: Omit<PositionsTableProps, "report" | "loading" | "error"> & {
  report: PositionsTableData;
}) {
  const rows = report.positionIds.map((id) => ({
    id,
    value: report.valuation.position_values[id],
  }));
  const [selected, setSelected] = useState(
    initialPosition ?? report.positionIds[0],
  );
  const position = selected
    ? report.valuation.position_values[selected]
    : undefined;
  const result = position?.valuation_result;
  return (
    <section
      aria-label="Positions"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <FinstackTable
        data={rows}
        columns={columns}
        getRowId={(row) => row.id}
        caption="Portfolio positions in spec order; activate a row for its valuation"
        captionVisibility="sr-only"
        density={density}
        emptyState="No positions returned"
        onRowActivate={(row) => setSelected(row.id)}
      />
      {selected !== undefined && (
        <section aria-label={`Position ${selected}`} className="space-y-3">
          <h3 className="text-sm font-medium">
            Position <span className="font-mono">{selected}</span>
          </h3>
          {position?.risk_error ? (
            <p role="note" className="text-xs text-muted-foreground">
              Risk error: {position.risk_error}
            </p>
          ) : null}
          {result ? (
            <>
              <ValuationSummary
                result={result}
                model={result.provenance?.model}
                density={density}
              />
              <MeasuresGrid
                result={result}
                content="measures"
                metadata={metadata}
                density={density}
              />
            </>
          ) : (
            <p role="status" className="text-muted-foreground">
              No valuation result returned for this position
            </p>
          )}
        </section>
      )}
    </section>
  );
}
