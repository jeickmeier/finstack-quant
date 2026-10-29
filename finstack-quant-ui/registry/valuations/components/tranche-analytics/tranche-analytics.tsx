"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type { ReactNode } from "react";
import type { OasResult, TrancheMetrics } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MeasureValue } from "@/components/finstack/valuations/primitives/measure-value/measure-value";

/** One native result tile: its value, its own native error, or a pending state. */
export interface TrancheTile<T> {
  value?: T | null;
  error?: string;
  loading?: boolean;
}
type Unit = { label: string; source: string };
type Row = { id: string; value: number | null | undefined; unit?: Unit };
/** Units as each Rust `TrancheMetrics` field documents them; values are never rescaled. */
function fieldUnit(
  field: keyof TrancheMetrics,
  currency: string,
): Unit | undefined {
  const source = `valuations::TrancheMetrics::${field}`;
  switch (field) {
    case "pv":
    case "accrued":
      return { label: currency, source };
    case "cs01":
      return { label: `${currency} per bp`, source };
    case "clean_price_pct":
    case "dirty_price_pct":
    case "target_price_pct":
      return { label: "% of current balance", source };
    case "wal":
    case "wal_to_call":
    case "spread_duration":
    case "effective_duration":
      return { label: "years", source };
    case "spread_convexity":
    case "effective_convexity":
      return { label: "years²", source };
    case "ytm":
      return { label: "decimal", source };
    case "z_spread_bp":
    case "z_spread_to_call_bp":
    case "dm_bp":
    case "dm_to_call_bp":
      return { label: "bp", source };
    case "factor":
      return { label: "current / original balance", source };
    default:
      return undefined;
  }
}
/** Scalar TrancheMetrics fields in the order the Rust struct declares them. */
const metricFields = [
  "pv",
  "clean_price_pct",
  "dirty_price_pct",
  "accrued",
  "factor",
  "wal",
  "ytm",
  "z_spread_bp",
  "cs01",
  "spread_duration",
  "spread_convexity",
  "effective_duration",
  "effective_convexity",
  "target_price_pct",
  "wal_to_call",
  "z_spread_to_call_bp",
  "dm_to_call_bp",
  "dm_bp",
] as const satisfies readonly (keyof TrancheMetrics)[];
/** Present fields only; a field Rust leaves unset for this tranche is not shown. */
function metricRows(metrics: TrancheMetrics): Row[] {
  const fields: Row[] = metricFields
    .filter((field) => metrics[field] !== undefined)
    .map((field) => ({
      id: field,
      value: metrics[field],
      unit: fieldUnit(field, metrics.currency),
    }));
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
  /** Native trancheBreakevenCdr result (decimal). */
  breakevenCdr: TrancheTile<number>;
  /** Native trancheDiscountMargin result at the price (decimal). */
  discountMargin: TrancheTile<number>;
  /** Native trancheOas result at the price under Rust's default OasConfig. */
  oas: TrancheTile<OasResult>;
  /** Request the OAS solve; omit when the host supplies the result. */
  onSolveOas?: () => void;
  density?: "compact" | "comfortable";
}
/**
 * Presents one structured-credit tranche's native analytics: TrancheMetrics,
 * break-even CDR, discount margin and OAS, each with its own native error.
 * Units are the ones each Rust field documents; nothing is computed, rescaled
 * or defaulted here, and no price is assumed.
 */
export function TrancheAnalytics({
  trancheId,
  price,
  onPriceChange,
  metrics,
  breakevenCdr,
  discountMargin,
  oas,
  onSolveOas,
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
            {metrics.value && (
              <FinstackTable
                data={metricRows(metrics.value)}
                columns={columns}
                getRowId={(row) => row.id}
                caption={`${trancheId} tranche metrics`}
                captionVisibility="sr-only"
                density={density}
              />
            )}
          </TileState>
        </TileFrame>
        <div className="flex min-w-0 flex-col gap-3">
          <TileFrame title="Break-even CDR">
            <TileState tile={breakevenCdr} idle="No break-even CDR returned">
              <MeasureValue
                value={breakevenCdr.value}
                unit={{
                  label: "decimal",
                  source: "valuations.instruments.trancheBreakevenCdr",
                }}
              />
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
              <MeasureValue
                value={discountMargin.value}
                unit={{
                  label: "decimal",
                  source: "valuations.instruments.trancheDiscountMargin",
                }}
              />
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
            <TileState
              tile={oas}
              idle={
                noPrice
                  ? "Enter a clean price to solve the OAS"
                  : "OAS runs on request"
              }
            >
              {oas.value && (
                <dl className="flex flex-col gap-1.5 text-sm">
                  {(
                    [
                      ["oas", "decimal"],
                      ["model_price", "% of current balance"],
                      ["market_price", "% of current balance"],
                      ["num_paths", undefined],
                      ["price_std_error", "% of current balance"],
                    ] as const
                  ).map(([field, unit]) => (
                    <div key={field} className="flex flex-col">
                      <dt className="text-xs text-muted-foreground">
                        {labelFor(field)}
                      </dt>
                      <dd>
                        <MeasureValue
                          value={oas.value![field] as number}
                          unit={
                            unit && {
                              label: unit,
                              source: `valuations::OasResult::${field}`,
                            }
                          }
                          showUnavailableUnit={false}
                        />
                      </dd>
                    </div>
                  ))}
                </dl>
              )}
            </TileState>
          </TileFrame>
        </div>
      </div>
    </section>
  );
}
