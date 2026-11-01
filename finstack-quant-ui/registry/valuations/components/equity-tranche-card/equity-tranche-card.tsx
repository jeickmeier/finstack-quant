"use client";
import type { ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  EquityMetricsFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MeasureValue } from "@/components/finstack/valuations/primitives/measure-value/measure-value";

type NativeMetrics = ReturnType<StructuredCredit["equityMetrics"]>;
type Distribution = NativeMetrics["cash_on_cash"][number];
function Tile({ field, children }: { field: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2">
      <dt className="text-xs font-medium text-muted-foreground">
        {labelFor(field)}
      </dt>
      <dd className="m-0 text-sm">{children}</dd>
    </div>
  );
}
function columns(
  unit: EquityMetricsFieldUnits["cash_on_cash"],
): ColumnDef<{}, Distribution, any>[] {
  return [
    {
      id: "payment_date",
      header: labelFor("payment_date"),
      accessorFn: ([date]) => date,
      meta: { className: "", rowHeader: true },
    },
    {
      id: unit.field,
      header: () => (
        <span title={unit.source}>
          {labelFor(unit.field)} ({unit.label})
        </span>
      ),
      accessorFn: ([, share]) => formatRaw(share),
      meta: { className: "text-right finstack-numeric" },
    },
  ];
}
export interface EquityTrancheCardProps {
  /** Native StructuredCredit.equityMetrics result; null while none is available. */
  metrics: NativeMetrics | null;
  /** Native equityMetricsFieldUnits for the result's currency; null while none is available. */
  units: EquityMetricsFieldUnits | null;
  /** Purchase price as typed, percent of the equity's current balance; empty leaves it to Rust. */
  purchasePrice: string;
  /** Omit for a read-only price. */
  onPurchasePriceChange?: (value: string) => void;
  loading?: boolean;
  /** The native error of the metrics or their units, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * The deal's equity tranche as Rust measured it: invested amount, IRR, MOIC, NAV and
 * each distribution's cash-on-cash. Rust chooses the tranche, and a deal without one
 * shows Rust's error. A missing IRR is stated and never drawn as zero; a zero Rust
 * returned is drawn as zero. Nothing is computed, rescaled or classified.
 */
export function EquityTrancheCard({
  metrics,
  units,
  purchasePrice,
  onPurchasePriceChange,
  loading,
  error,
  density,
}: EquityTrancheCardProps) {
  return (
    <section
      aria-label="Equity tranche"
      aria-busy={loading}
      className="flex min-w-0 flex-col gap-3 font-sans text-foreground"
      data-density={density}
    >
      <div className="flex flex-wrap items-end gap-3">
        {metrics && !error && (
          <h3 className="text-sm font-medium">
            Tranche <span className="font-mono">{metrics.tranche_id}</span>
          </h3>
        )}
        <div className="w-56">
          <DecimalInput
            label="Purchase price"
            help="Blank invests at Rust's default, par"
            value={purchasePrice}
            onValueChange={onPurchasePriceChange ?? (() => {})}
            readOnly={!onPurchasePriceChange}
            allowNegative={false}
            suffix="% of current balance"
          />
        </div>
      </div>
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !metrics ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Calculating…" : "No equity metrics returned"}
        </p>
      ) : !units ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Calculating…" : "No units returned"}
        </p>
      ) : (
        <>
          <dl className="m-0 grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-2">
            <Tile field={units.invested.field}>
              <MeasureValue value={metrics.invested} unit={units.invested} />
            </Tile>
            <Tile field={units.irr.field}>
              {metrics.irr === null || metrics.irr === undefined ? (
                <span className="finstack-na" title={units.irr.source}>
                  No rate solves
                </span>
              ) : (
                <MeasureValue value={metrics.irr} unit={units.irr} />
              )}
            </Tile>
            <Tile field={units.moic.field}>
              <MeasureValue value={metrics.moic} unit={units.moic} />
            </Tile>
            <Tile field={units.nav_pct.field}>
              <MeasureValue value={metrics.nav_pct} unit={units.nav_pct} />
            </Tile>
          </dl>
          <FinstackTable
            caption={labelFor(units.cash_on_cash.field)}
            data={metrics.cash_on_cash}
            columns={columns(units.cash_on_cash)}
            getRowId={(row) => String(metrics.cash_on_cash.indexOf(row))}
            emptyState="Rust returned no distributions after the valuation date"
            density={density}
          />
        </>
      )}
    </section>
  );
}
