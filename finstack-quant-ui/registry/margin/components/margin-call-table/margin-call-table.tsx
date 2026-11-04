"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  MarginCall,
  MoneyValue as Money,
  VmResult,
} from "finstack-quant-wasm";
import { Badge } from "@/components/ui/badge";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { ReportSection } from "@/components/finstack/shared/primitives/native-display/native-display";

/** A returned VmResult with Rust's own call decision and net margin; the shape useVm returns. */
export interface VmMarginReport {
  readonly result: VmResult;
  /** margin.vmResultRequiresCall of `result`. */
  readonly requiresCall: boolean;
  /** margin.vmResultNetMargin of `result`: post minus collect in major units of the CSA base currency. */
  readonly netMargin: number;
}
export interface MarginCallTableProps {
  /** The returned variation margin of one date; null or omitted hides the section. */
  vm?: VmMarginReport | null;
  /** The returned VmCalculator#generateMarginCalls schedule; null or omitted hides the section. */
  calls?: readonly MarginCall[] | null;
  /** The returned VmCalculator#marginCallDates; null or omitted hides the section. */
  callDates?: readonly string[] | null;
  loading?: boolean;
  /** The native error message, shown instead of the tables. */
  error?: string;
  /** Display text of the returned net margin; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/** VmResult money fields in the order the calculation applies them. */
const vmFields = [
  "gross_exposure",
  "threshold",
  "independent_amount",
  "net_exposure",
  "collateral_balance",
  "unrounded_call",
  "mta",
  "rounding_increment",
  "post_amount",
  "collect_amount",
] as const;
type VmRow = { field: (typeof vmFields)[number]; amount: Money };
const vmColumns: ColumnDef<{}, VmRow, any>[] = [
  {
    id: "field",
    header: "Term",
    accessorFn: (row) => labelFor(row.field),
    meta: { className: "text-left" },
  },
  {
    id: "amount",
    header: labelFor("amount"),
    accessorFn: (row) => row.amount.amount,
    cell: ({ row }) => <MoneyValue value={row.original.amount} />,
    meta: { className: "text-right" },
  },
];
type CallRow = MarginCall & { index: number };
const callMoney = [
  "amount",
  "mtm_trigger",
  "threshold",
  "mta_applied",
] as const;
const callColumns: ColumnDef<{}, CallRow, any>[] = [
  {
    id: "call_date",
    header: labelFor("call_date"),
    accessorFn: (row) => row.call_date,
    meta: { className: "text-left finstack-numeric" },
  },
  {
    id: "settlement_date",
    header: labelFor("settlement_date"),
    accessorFn: (row) => row.settlement_date,
    meta: { className: "text-left finstack-numeric" },
  },
  {
    id: "call_type",
    header: labelFor("call_type"),
    accessorFn: (row) => labelFor(row.call_type),
    meta: { className: "text-left" },
  },
  ...callMoney.map((field): ColumnDef<{}, CallRow, any> => ({
    id: field,
    header: labelFor(field),
    accessorFn: (row) => row[field].amount,
    cell: ({ row }) => <MoneyValue value={row.original[field]} />,
    meta: { className: "text-right" },
  })),
];
/**
 * Presents native variation margin: one date's VmResult with Rust's call
 * decision and net margin, a margin-call schedule and the contractual call
 * dates. Every amount, date and decision is shown as returned; nothing is
 * netted, thresholded or rounded here.
 */
export function MarginCallTable({
  vm,
  calls,
  callDates,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: MarginCallTableProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Calculating variation margin…
      </p>
    );
  if (!vm && !calls && !callDates)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No variation margin result
      </p>
    );
  return (
    <section
      aria-label="Variation margin and calls"
      data-density={density}
      className="space-y-6 font-sans text-sm text-foreground"
    >
      {vm ? (
        <ReportSection title="Variation margin">
          <div className="flex flex-wrap items-center gap-3">
            <Badge variant={vm.requiresCall ? "default" : "secondary"}>
              {vm.requiresCall ? "Margin call" : "No margin call"}
            </Badge>
            <span aria-label="Net margin" className="finstack-numeric">
              Net margin {vm.result.post_amount.currency}{" "}
              {formatValue(vm.netMargin)}
            </span>
            <span className="text-muted-foreground">
              (post minus collect; positive means the desk posts)
            </span>
          </div>
          <p className="text-muted-foreground">
            As of {vm.result.date} · settles {vm.result.settlement_date}
          </p>
          <FinstackTable
            data={vmFields.map((field) => ({
              field,
              amount: vm.result[field],
            }))}
            columns={vmColumns}
            getRowId={(row) => row.field}
            caption="Variation margin terms and amounts"
            captionVisibility="sr-only"
            density={density}
          />
        </ReportSection>
      ) : null}
      {calls ? (
        <ReportSection title="Margin calls">
          <FinstackTable
            data={calls.map((call, index) => ({ ...call, index }))}
            columns={callColumns}
            getRowId={(row) => String(row.index)}
            caption="Margin calls in exposure order"
            captionVisibility="sr-only"
            density={density}
            emptyState="No margin calls"
          />
        </ReportSection>
      ) : null}
      {callDates ? (
        <ReportSection title="Call dates">
          {callDates.length ? (
            <ol className="flex flex-wrap gap-x-4 gap-y-1 finstack-numeric">
              {callDates.map((date) => (
                <li key={date}>{date}</li>
              ))}
            </ol>
          ) : (
            <p role="status" className="text-muted-foreground">
              No call dates
            </p>
          )}
        </ReportSection>
      ) : null}
    </section>
  );
}
