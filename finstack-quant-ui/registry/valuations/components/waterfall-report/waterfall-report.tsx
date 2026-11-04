"use client";
import { useState } from "react";
import type { ColumnDef, RowData } from "@tanstack/react-table";
import type { StructuredCredit } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  UndefinedValue,
  yesNo,
} from "@/components/finstack/shared/primitives/native-display/native-display";

type SimulationRun = ReturnType<StructuredCredit["runSimulation"]>;
type Period = NonNullable<SimulationRun["diagnostics"]["periods"]>[number];
type Waterfall = Period["waterfall"];
type Money = Waterfall["total_available"];
type TierAllocation = Waterfall["tier_allocations"][number];
type PaymentRecord = Waterfall["payment_records"][number];
type Diversion = NonNullable<Waterfall["diverted_amounts"]>[number];
type NonWaterfallPayment = Period["non_waterfall_payments"][number];

/** The WaterfallDistribution amounts a period row shows, as cash in, then cash moved, then cash carried. */
const amounts = [
  "total_available",
  "recovery_proceeds",
  "diverted_cash",
  "remaining_interest",
  "remaining_principal",
] as const;
const headers: { id: string; label?: string; className?: string }[] = [
  { id: "payment_date" },
  ...amounts.map((id) => ({ id, className: "text-right" })),
  { id: "had_diversions" },
  {
    id: "non_waterfall_payments",
    label: `${labelFor("non_waterfall_payments")} (count)`,
    className: "text-right",
  },
];
const numeric = { className: "text-right finstack-numeric" };
/** A detail column over one field of a row, headed by the field's name. */
function column<Row extends RowData>(
  id: string,
  read: (row: Row) => string | number,
  meta?: { className: string },
): ColumnDef<{}, Row, any> {
  return { id, header: labelFor(id), accessorFn: read, meta };
}
function moneyColumn<Row extends RowData>(
  id: string,
  read: (row: Row) => Money,
): ColumnDef<{}, Row, any> {
  return column(id, (row) => groupDecimal(read(row).amount), numeric);
}
const tierColumns: ColumnDef<{}, TierAllocation, any>[] = [
  column("tier", ([tier]) => tier),
  moneyColumn("amount", ([, amount]) => amount),
];
const recordColumns: ColumnDef<{}, PaymentRecord, any>[] = [
  column("tier_id", (row) => row.tier_id),
  column("recipient_id", (row) => row.recipient_id),
  column("priority", (row) => row.priority, numeric),
  moneyColumn("requested_amount", (row) => row.requested_amount),
  moneyColumn("paid_amount", (row) => row.paid_amount),
  moneyColumn("shortfall", (row) => row.shortfall),
  column("is_principal", (row) => yesNo(row.is_principal)),
  column("diverted", (row) => yesNo(row.diverted)),
];
const diversionColumns: ColumnDef<{}, Diversion, any>[] = [
  column("source_tier", (row) => row.source_tier),
  column("target_tranche", (row) => row.target_tranche),
  moneyColumn("amount", (row) => row.amount),
  column("reason", (row) => row.reason, { className: "whitespace-normal" }),
];
const nonWaterfallColumns: ColumnDef<{}, NonWaterfallPayment, any>[] = [
  column("payment_date", (row) => row.payment_date),
  column("tranche_id", (row) => row.tranche_id),
  column("source", (row) => labelFor(row.source)),
  moneyColumn("interest", (row) => row.interest),
  moneyColumn("principal", (row) => row.principal),
  moneyColumn("incentive_fee", (row) => row.incentive_fee),
];
/** Rust's rows carry no id of their own, and may repeat a tier, so a row is its position. */
function DetailTable<Row extends RowData>({
  caption,
  rows,
  columns,
  density,
}: {
  caption: string;
  rows: Row[];
  columns: ColumnDef<{}, Row, any>[];
  density?: "compact" | "comfortable";
}) {
  return (
    <FinstackTable
      caption={caption}
      data={rows}
      columns={columns}
      getRowId={(row) => String(rows.indexOf(row))}
      emptyState="None this period"
      density={density}
    />
  );
}
function PeriodRows({
  period,
  density,
}: {
  period: Period;
  density?: "compact" | "comfortable";
}) {
  const [open, setOpen] = useState(false);
  const { waterfall } = period;
  return (
    <>
      <TableRow>
        <TableHead
          scope="row"
          className="h-[var(--row-height)] py-0 font-normal"
        >
          <Button
            variant="ghost"
            size="xs"
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            <span aria-hidden="true">{open ? "▾" : "▸"}</span>
            {period.payment_date}
          </Button>
        </TableHead>
        {amounts.map((field) => {
          const money = waterfall[field];
          return (
            <TableCell
              key={field}
              className="h-[var(--row-height)] py-0 text-right finstack-numeric"
            >
              {money ? groupDecimal(money.amount) : <UndefinedValue />}
            </TableCell>
          );
        })}
        <TableCell className="h-[var(--row-height)] py-0">
          {yesNo(waterfall.had_diversions)}
        </TableCell>
        <TableCell className="h-[var(--row-height)] py-0 text-right finstack-numeric">
          {period.non_waterfall_payments.length}
        </TableCell>
      </TableRow>
      {open && (
        <TableRow>
          <TableCell colSpan={headers.length} className="whitespace-normal p-2">
            {/* Zero width, full minimum: the nested tables scroll inside this cell instead of widening the period table. */}
            <div className="flex w-0 min-w-full flex-col gap-3">
              <p className="m-0 text-sm">
                {labelFor("principal_used_for_interest")}:{" "}
                <span className="finstack-numeric">
                  {groupDecimal(waterfall.principal_used_for_interest.amount)}
                </span>
              </p>
              <DetailTable
                caption="Tier allocations"
                rows={waterfall.tier_allocations}
                columns={tierColumns}
                density={density}
              />
              <DetailTable
                caption="Payment records"
                rows={waterfall.payment_records}
                columns={recordColumns}
                density={density}
              />
              {waterfall.diverted_amounts ? (
                <DetailTable
                  caption="Diversions"
                  rows={waterfall.diverted_amounts}
                  columns={diversionColumns}
                  density={density}
                />
              ) : (
                <p className="m-0 text-sm">
                  Diversions: <UndefinedValue />
                </p>
              )}
              <DetailTable
                caption="Non-waterfall payments"
                rows={period.non_waterfall_payments}
                columns={nonWaterfallColumns}
                density={density}
              />
            </div>
          </TableCell>
        </TableRow>
      )}
    </>
  );
}
export interface WaterfallReportProps {
  /** Native StructuredCredit.runSimulation result; null while none is available. */
  run: SimulationRun | null;
  loading?: boolean;
  /** The native error of the simulation, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * One row per simulated period, as Rust returned them and in payment order, with
 * the period's tier allocations, payment records, diversions and non-waterfall
 * payments on expand. A deal with no payment period left has one row, its
 * settlement, whose release is in that row's detail. Nothing is summed, joined by
 * date, sorted or classified.
 */
export function WaterfallReport({
  run,
  loading,
  error,
  density,
}: WaterfallReportProps) {
  if (error)
    return (
      <p role="alert" className="text-sm text-error">
        {error}
      </p>
    );
  if (!run)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Calculating…" : "No simulation returned"}
      </p>
    );
  const periods = run.diagnostics.periods ?? [];
  if (periods.length === 0)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        The simulation returned no periods
      </p>
    );
  return (
    <section
      aria-label="Waterfall report"
      aria-busy={loading}
      className="min-w-0 font-sans text-base text-foreground"
      data-density={density}
    >
      <Table>
        {/* Every amount of a period is in the deal currency, per WaterfallDistribution and NonWaterfallPayment. */}
        <TableCaption>
          By payment period, amounts in{" "}
          {periods[0]!.waterfall.total_available.currency}
        </TableCaption>
        <TableHeader>
          <TableRow>
            {headers.map(({ id, label, className }) => (
              <TableHead
                key={id}
                scope="col"
                className={`h-[var(--row-height)] whitespace-normal py-0 text-xs font-medium tracking-wide text-muted-foreground ${className ?? ""}`}
              >
                {label ?? labelFor(id)}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {periods.map((period) => (
            <PeriodRows
              key={period.payment_date}
              period={period}
              density={density}
            />
          ))}
        </TableBody>
      </Table>
      {periods.some((period) => period.non_waterfall_payments[0]) && (
        <p className="mt-2 mb-0 text-sm text-muted-foreground">
          The amounts above are the waterfall's. Cash paid outside it, such as a
          settlement's release, is listed when a period with a non-waterfall
          count is expanded.
        </p>
      )}
    </section>
  );
}
