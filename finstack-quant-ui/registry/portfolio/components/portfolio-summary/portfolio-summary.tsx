"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  MoneyValue as Money,
  PortfolioValuation,
} from "finstack-quant-wasm";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** The parts of a returned portfolio valuation the summary presents. */
export interface PortfolioSummaryData {
  /** Entity ids in registration order (Portfolio#entityIds); `by_entity` rows follow it. */
  entityIds: readonly string[];
  /** portfolio.valuePortfolio as returned. */
  valuation: PortfolioValuation;
}
export interface PortfolioSummaryProps {
  /** The returned valuation; null while none is available. */
  report: PortfolioSummaryData | null;
  loading?: boolean;
  /** The native error message, shown instead of a summary. */
  error?: string;
  density?: "compact" | "comfortable";
}
type EntityRow = { id: string; value: Money | undefined };
const entityColumns: ColumnDef<{}, EntityRow, any>[] = [
  {
    id: "entity",
    header: "Entity",
    accessorFn: (row) => row.id,
    meta: { className: "text-left font-mono" },
  },
  {
    id: "value",
    header: "Value (base currency)",
    accessorFn: (row) => row.value?.amount,
    cell: ({ row }) => <MoneyValue value={row.original.value} />,
    meta: { className: "text-right" },
  },
];
/**
 * Presents a native portfolio valuation's totals: the base-currency total,
 * the by-entity values in entity registration order, the FX collapse policy
 * Rust stamped (as text) and the positions whose risk fell back to PV only.
 * Every amount is shown as returned; nothing is summed or converted.
 */
export function PortfolioSummary({
  report,
  loading,
  error,
  density = "compact",
}: PortfolioSummaryProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Valuing portfolio…
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No portfolio valuation
      </p>
    );
  const { valuation, entityIds } = report;
  const degraded = valuation.degraded_positions ?? [];
  // Registration order first; an entity Rust returned but the ids omit follows in returned order.
  const ids = [
    ...entityIds.filter((id) => id in valuation.by_entity),
    ...Object.keys(valuation.by_entity).filter((id) => !entityIds.includes(id)),
  ];
  return (
    <section
      aria-label="Portfolio summary"
      data-density={density}
      className="@container space-y-4 font-sans text-sm text-foreground"
    >
      <section aria-label="Portfolio total" className="space-y-1">
        <h3 className="text-xs font-medium text-muted-foreground">
          Total value (base currency)
        </h3>
        <MoneyValue value={valuation.total_base_currency} prominent />
        <dl className="flex flex-wrap gap-x-6 gap-y-1 text-xs text-muted-foreground">
          <div className="flex gap-1">
            <dt>Valuation date</dt>
            <dd className="font-mono text-foreground">{valuation.as_of}</dd>
          </div>
          <div className="flex gap-1">
            <dt>FX collapse policy</dt>
            <dd className="font-mono text-foreground">
              {String(valuation.fx_collapse_policy)}
            </dd>
          </div>
        </dl>
      </section>
      <section aria-label="Value by entity" className="space-y-2">
        <h3 className="text-sm font-medium">Value by entity</h3>
        <FinstackTable
          data={ids.map((id) => ({ id, value: valuation.by_entity[id] }))}
          columns={entityColumns}
          getRowId={(row) => row.id}
          caption="Portfolio value by entity in the base currency"
          captionVisibility="sr-only"
          density={density}
          emptyState="No entity values returned"
        />
      </section>
      <section aria-label="Degraded positions" className="space-y-2">
        <h3 className="text-sm font-medium">Degraded positions</h3>
        {degraded.length ? (
          <ul className="space-y-1">
            {degraded.map((id) => (
              <li key={id}>
                <span className="font-mono">{id}</span>
                {valuation.position_values[id]?.risk_error ? (
                  <span className="text-muted-foreground">
                    {" "}
                    · {valuation.position_values[id]?.risk_error}
                  </span>
                ) : null}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted-foreground">
            No position fell back to PV only.
          </p>
        )}
      </section>
    </section>
  );
}
