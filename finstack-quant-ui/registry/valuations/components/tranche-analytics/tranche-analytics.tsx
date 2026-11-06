"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type { ReactNode } from "react";
import type {
  MetricFieldUnit,
  OasResult,
  TableEnvelope as NativeTable,
  TrancheCashflows,
  TrancheSolveFieldUnits,
  TrancheMetrics,
} from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { formatRawMoney } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";
import { MeasureValue } from "@/components/finstack/valuations/primitives/measure-value/measure-value";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { oasConfigModule } from "./oas-config";

/** One native result tile: its value, its own native error, or a pending state. */
export interface TrancheTile<T> {
  value?: T | null;
  error?: string;
  loading?: boolean;
}
type Row = {
  id: string;
  value: number | null | undefined;
  unit?: Pick<MetricFieldUnit, "label" | "source">;
};
/** Rust's scalar fields in its order; a field Rust leaves unset for this tranche is not shown. */
function metricRows(
  metrics: TrancheMetrics,
  units: readonly MetricFieldUnit[],
): Row[] {
  const fields: Row[] = units
    .map((unit) => ({
      id: unit.field,
      value: metrics[unit.field as keyof TrancheMetrics] as number | undefined,
      unit,
    }))
    .filter((row) => row.value !== undefined);
  // Requested registry metrics keep their native units, which the result does not state.
  const registry: Row[] = Object.entries(metrics.metrics ?? {}).map(
    ([id, value]) => ({ id, value }),
  );
  return [...fields, ...registry];
}
const columns: ColumnDef<{}, Row, any>[] = [
  {
    id: "measure",
    header: "Measure",
    accessorFn: (row) => labelFor(row.id),
    meta: { className: "", rowHeader: true },
  },
  {
    id: "value",
    header: "Value",
    accessorFn: (row) => row.value,
    cell: ({ row }) => (
      <MeasureValue value={row.original.value} unit={row.original.unit} />
    ),
    meta: { className: "text-right finstack-numeric" },
  },
];
/** TrancheCashflows' scalar totals, in the struct's order; each is shown as returned. */
const scheduleTotals = [
  "total_interest",
  "total_principal",
  "total_pik",
  "total_deferred",
  "total_writedown",
  "final_balance",
] as const;
/** Native flows of one tranche and Rust's projection of them into one row per date. */
export interface TrancheScheduleValue {
  cashflows: TrancheCashflows;
  table: NativeTable;
}
function TileFrame({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section
      aria-label={title}
      className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
    >
      <h4 className="text-xs font-medium text-muted-foreground">{title}</h4>
      {children}
    </section>
  );
}
function TileState({
  tile,
  idle,
  children,
}: {
  tile: TrancheTile<unknown>;
  idle: string;
  children: ReactNode;
}) {
  if (tile.error)
    return (
      <p role="alert" className="text-sm text-error">
        {tile.error}
      </p>
    );
  if (tile.loading)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        Calculating…
      </p>
    );
  if (tile.value == null)
    return <p className="text-sm text-muted-foreground">{idle}</p>;
  return <>{children}</>;
}
export interface TrancheAnalyticsProps {
  trancheId: string;
  /** Clean settlement price as typed, percent of the tranche's current balance; empty means none. */
  price: string;
  /** Omit for a read-only price. */
  onPriceChange?: (value: string) => void;
  /** Native trancheMetrics result; solved against the price when one is supplied, else Rust's default target. */
  metrics: TrancheTile<TrancheMetrics>;
  /** Native trancheMetricFieldUnits for the result's currency; connect useTrancheMetricFieldUnits. */
  metricUnits: TrancheTile<readonly MetricFieldUnit[]>;
  /** Native trancheSolveFieldUnits, the units of the three tiles below; connect useTrancheSolveFieldUnits. */
  solveUnits: TrancheTile<TrancheSolveFieldUnits>;
  /** Native trancheBreakevenCdr result. */
  breakevenCdr: TrancheTile<number>;
  /** Native trancheDiscountMargin result at the price. */
  discountMargin: TrancheTile<number>;
  /** Native trancheOas result at the price, under the applied OasConfig or Rust's default. */
  oas: TrancheTile<OasResult>;
  /** Request the OAS solve; omit when the host supplies the result. */
  onSolveOas?: () => void;
  /**
   * Receives complete OasConfig JSON when the OAS configuration form is applied.
   * The form starts from the schema's Rust example (`OasConfig::default()`); omit
   * this to hide the form and leave the configuration to the host.
   */
  onOasConfigChange?: (json: string) => void;
  /** Native trancheCashflows and trancheCashflowsTable; omit to hide the schedule. Rust joins and sums the dates. */
  schedule?: TrancheTile<TrancheScheduleValue>;
  density?: "compact" | "comfortable";
}
/**
 * Presents one structured-credit tranche's native analytics: TrancheMetrics,
 * break-even CDR, discount margin and OAS, each with its own native error, and
 * optionally an editor for the native OasConfig.
 * Metric units are Rust's own; nothing is computed, rescaled
 * or defaulted here, and no price is assumed.
 */
export function TrancheAnalytics({
  trancheId,
  price,
  onPriceChange,
  metrics,
  metricUnits,
  solveUnits,
  breakevenCdr,
  discountMargin,
  oas,
  onSolveOas,
  onOasConfigChange,
  schedule,
  density,
}: TrancheAnalyticsProps) {
  const noPrice = price.trim() === "";
  return (
    <section
      aria-label={`${trancheId} tranche analytics`}
      className="flex flex-col gap-3 font-sans text-foreground"
      data-density={density}
    >
      <div className="flex flex-wrap items-end gap-3">
        <h3 className="text-sm font-medium">
          Tranche <span className="font-mono">{trancheId}</span>
        </h3>
        <div className="w-56">
          <DecimalInput
            label="Clean price"
            value={price}
            onValueChange={onPriceChange ?? (() => {})}
            readOnly={!onPriceChange}
            allowNegative={false}
            suffix="% of current balance"
          />
        </div>
      </div>
      <div className="grid gap-3 md:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
        <TileFrame title="Tranche metrics">
          <TileState tile={metrics} idle="No tranche metrics returned">
            <TileState tile={metricUnits} idle="No field units returned">
              {metrics.value && metricUnits.value && (
                <FinstackTable
                  data={metricRows(metrics.value, metricUnits.value)}
                  columns={columns}
                  getRowId={(row) => row.id}
                  caption={`${trancheId} tranche metrics`}
                  captionVisibility="sr-only"
                  density={density}
                />
              )}
            </TileState>
          </TileState>
        </TileFrame>
        <div className="flex min-w-0 flex-col gap-3">
          <TileFrame title="Break-even CDR">
            <TileState tile={breakevenCdr} idle="No break-even CDR returned">
              <TileState tile={solveUnits} idle="No units returned">
                <MeasureValue
                  value={breakevenCdr.value}
                  unit={solveUnits.value?.breakeven_cdr}
                />
              </TileState>
            </TileState>
          </TileFrame>
          <TileFrame title="Discount margin">
            <TileState
              tile={discountMargin}
              idle={
                noPrice
                  ? "Enter a clean price to solve the discount margin"
                  : "No discount margin returned"
              }
            >
              <TileState tile={solveUnits} idle="No units returned">
                <MeasureValue
                  value={discountMargin.value}
                  unit={solveUnits.value?.discount_margin}
                />
              </TileState>
            </TileState>
          </TileFrame>
          <TileFrame title="Option-adjusted spread">
            {onSolveOas && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="self-start"
                disabled={noPrice || oas.loading}
                onClick={onSolveOas}
              >
                Solve OAS
              </Button>
            )}
            {onOasConfigChange && (
              <details className="text-sm">
                <summary>OAS configuration</summary>
                <p className="text-xs text-muted-foreground">
                  Starts from Rust&apos;s default OasConfig; applied to the next
                  solve.
                </p>
                <SchemaForm
                  module={oasConfigModule}
                  layout="full"
                  submitLabel="Apply configuration"
                  submitVariant="outline"
                  validate={async (json) => json}
                  onSubmit={onOasConfigChange}
                />
              </details>
            )}
            <TileState
              tile={oas}
              idle={
                noPrice
                  ? "Enter a clean price to solve the OAS"
                  : "OAS runs on request"
              }
            >
              <TileState tile={solveUnits} idle="No units returned">
                {oas.value && solveUnits.value && (
                  <dl className="flex flex-col gap-1.5 text-sm">
                    {solveUnits.value.oas.map((unit) => (
                      <div key={unit.field} className="flex flex-col">
                        <dt className="text-xs text-muted-foreground">
                          {labelFor(unit.field)}
                        </dt>
                        <dd>
                          <MeasureValue
                            value={
                              oas.value![
                                unit.field as keyof OasResult
                              ] as number
                            }
                            unit={unit}
                          />
                        </dd>
                      </div>
                    ))}
                  </dl>
                )}
              </TileState>
            </TileState>
          </TileFrame>
        </div>
      </div>
      {schedule && (
        <TileFrame title="Cash flows by date">
          <TileState tile={schedule} idle="No cash flows returned">
            {schedule.value && (
              <div className="flex flex-col gap-2">
                <dl className="flex flex-wrap gap-x-6 gap-y-1 text-sm">
                  {scheduleTotals.map((field) => (
                    <div key={field} className="flex flex-col">
                      <dt className="text-xs text-muted-foreground">
                        {labelFor(field)}
                      </dt>
                      <dd className="finstack-numeric">
                        {formatRawMoney(schedule.value!.cashflows[field])}
                      </dd>
                    </div>
                  ))}
                </dl>
                <TableEnvelope
                  envelope={schedule.value.table}
                  caption={`${trancheId} cash flows by date, in ${schedule.value.cashflows.total_interest.currency}`}
                  density={density}
                  emptyState="No cash flows returned"
                />
              </div>
            )}
          </TileState>
        </TileFrame>
      )}
    </section>
  );
}
