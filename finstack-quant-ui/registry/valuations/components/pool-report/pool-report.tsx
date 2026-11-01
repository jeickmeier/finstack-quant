"use client";
import type { ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  MetricFieldUnit,
  PoolReportFieldUnits,
  valuations,
} from "finstack-quant-wasm";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MeasureValue } from "@/components/finstack/valuations/primitives/measure-value/measure-value";

type NativeReport = ReturnType<typeof valuations.instruments.poolReport>;
type Row = NativeReport["by_obligor"][number];
/** The dimensions PoolReport groups by; each names its `by_*` table. */
const dimensions = ["obligor", "industry", "rating"] as const;
/** A money amount's exact digits, or a number as returned. */
function display(value: Row["balance"] | number) {
  return typeof value === "number"
    ? formatRaw(value)
    : groupDecimal(value.amount);
}
/** Rust's key column, then its numeric fields in order, each headed by Rust's unit. */
function columns(
  dimension: string,
  units: readonly MetricFieldUnit[],
): ColumnDef<{}, Row, any>[] {
  return [
    {
      id: "key",
      header: labelFor(dimension),
      accessorFn: (row) => row.key,
      cell: ({ getValue }) => {
        const key = getValue() as string | null | undefined;
        return key == null ? (
          <span
            className="finstack-na"
            title={`These assets carry no ${dimension}`}
          >
            Missing
          </span>
        ) : (
          key
        );
      },
      meta: { className: "", rowHeader: true },
    },
    ...units.map((unit): ColumnDef<{}, Row, any> => ({
      id: unit.field,
      header: () => (
        <span title={unit.source}>
          {labelFor(unit.field)} ({unit.label})
        </span>
      ),
      accessorFn: (row) => row[unit.field as keyof Row],
      cell: ({ getValue }) => display(getValue() as Row["balance"] | number),
      meta: { className: "text-right finstack-numeric" },
    })),
  ];
}
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
/** A returned number with its unit; Rust's null, an empty population, is missing and never zero. */
function Measure({
  value,
  unit,
}: {
  value: number | null | undefined;
  unit: MetricFieldUnit;
}) {
  return value == null ? (
    <span className="finstack-na" title={unit.source}>
      Missing
    </span>
  ) : (
    <MeasureValue value={value} unit={unit} />
  );
}
export interface PoolReportProps {
  /** Native valuations.instruments.poolReport result; null while none is available. */
  report: NativeReport | null;
  /** Native poolReportFieldUnits for the report's currency; connect usePoolReportFieldUnits. */
  units: PoolReportFieldUnits | null;
  loading?: boolean;
  /** The native error of the report or its units, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * A collateral pool's statistics, WARF and concentration by obligor, industry and
 * rating, as Rust returned them and in Rust's row order. A row with a null key
 * holds the assets missing that attribute, and a null statistic is shown as missing.
 * Nothing is summed, sorted or grouped.
 */
export function PoolReport({
  report,
  units,
  loading,
  error,
  density,
}: PoolReportProps) {
  if (error)
    return (
      <p role="alert" className="text-sm text-error">
        {error}
      </p>
    );
  if (!report || !units)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Calculating…" : "No pool report returned"}
      </p>
    );
  return (
    <section
      aria-label="Pool report"
      aria-busy={loading}
      className="flex flex-col gap-3 font-sans text-foreground"
      data-density={density}
    >
      <dl className="m-0 grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-2">
        <Tile field={units.total_balance.field}>
          <span className="finstack-numeric" title={units.total_balance.source}>
            {groupDecimal(report.total_balance.amount)}
            <span className="finstack-na ml-1 text-xs whitespace-nowrap">
              {units.total_balance.label}
            </span>
          </span>
        </Tile>
        <Tile field={units.warf.field}>
          <Measure value={report.warf} unit={units.warf} />
        </Tile>
        {units.stats.map((unit) => (
          <Tile key={unit.field} field={unit.field}>
            <Measure
              value={report.stats[unit.field as keyof typeof report.stats]}
              unit={unit}
            />
          </Tile>
        ))}
      </dl>
      {dimensions.map((dimension) => (
        <FinstackTable
          key={dimension}
          caption={labelFor(`by_${dimension}`)}
          data={report[`by_${dimension}`]}
          columns={columns(dimension, units.concentration)}
          // JSON keeps the null key apart from a name that reads "null".
          getRowId={(row) => JSON.stringify(row.key ?? null)}
          emptyState="No assets in the pool"
          density={density}
        />
      ))}
    </section>
  );
}
