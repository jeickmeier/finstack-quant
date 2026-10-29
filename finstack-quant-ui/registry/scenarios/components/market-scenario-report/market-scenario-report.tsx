"use client";
import type { ColumnDef } from "@tanstack/react-table";
import type {
  ApplicationReport,
  MoneyValue as Money,
} from "finstack-quant-wasm";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { StampBadge } from "@/components/finstack/valuations/primitives/stamp-badge/stamp-badge";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
export interface MarketScenarioReportProps {
  /** Native scenarios.applyScenario report, shown as returned. */
  report: ApplicationReport | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
type Shock = ApplicationReport["applied_shocks"] extends
  (infer Item)[] | undefined
  ? Item
  : never;
type Warning = ApplicationReport["warnings"][number];
type Carry = { id: string; money: Money };
/** A returned list entry and its position, which identifies the row. */
type Indexed<T> = { index: number; value: T };
const indexed = <T,>(values: readonly T[]): Indexed<T>[] =>
  values.map((value, index) => ({ index, value }));
/** Every field of a returned variant as `name value` text, in returned order; nothing is interpreted. */
function fieldsText(value: object, skip: readonly string[] = []) {
  return Object.entries(value)
    .filter(([key]) => !skip.includes(key))
    .map(
      ([key, field]) =>
        `${key} ${typeof field === "object" && field !== null ? JSON.stringify(field) : String(field)}`,
    )
    .join(" · ");
}
const shockColumns: ColumnDef<{}, Indexed<Shock>, any>[] = [
  {
    id: "target",
    header: "Target",
    accessorFn: ({ value }) =>
      value.applies_to.scope === "market"
        ? fieldsText(value.applies_to.target)
        : fieldsText(value.applies_to, ["scope"]),
    meta: { className: "text-left" },
  },
  {
    id: "scope",
    header: "Scope",
    accessorFn: ({ value }) => value.applies_to.scope,
    meta: { className: "text-left" },
  },
  {
    id: "shock",
    header: "Shock",
    accessorFn: ({ value }) => fieldsText(value.shock),
    meta: { className: "text-left" },
  },
];
const warningColumns: ColumnDef<{}, Indexed<Warning>, any>[] = [
  {
    id: "kind",
    header: "Warning",
    accessorFn: ({ value }) => value.kind,
    meta: { className: "text-left font-mono" },
  },
  {
    id: "detail",
    header: "Detail",
    accessorFn: ({ value }) => fieldsText(value, ["kind"]),
    meta: { className: "text-left" },
  },
];
const carryColumns: ColumnDef<{}, Carry, any>[] = [
  {
    id: "instrument",
    header: "Instrument",
    accessorFn: (row) => row.id,
    meta: { className: "text-left font-mono" },
  },
  {
    id: "carry",
    header: "Carry",
    accessorFn: (row) => row.money.amount,
    meta: { className: "text-right finstack-numeric" },
    cell: (context) => <MoneyValue value={context.row.original.money} />,
  },
];
/**
 * Display of a native market scenario application report: counters, applied
 * shocks, the time roll with its returned carry, every warning variant and the
 * policy stamp. Values are shown as returned; nothing is summed or converted.
 */
export function MarketScenarioReport({
  report,
  loading,
  error,
  density,
}: MarketScenarioReportProps) {
  if (error)
    return (
      <p role="alert" className="text-sm text-error">
        {error}
      </p>
    );
  if (!report)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Applying scenario…" : "No scenario applied"}
      </p>
    );
  const roll = report.time_roll;
  const carry: Carry[] = (roll?.instrument_carry ?? []).flatMap(
    ([id, amounts]) =>
      Object.values(amounts).map((money) => ({ id, money: money as Money })),
  );
  return (
    <div
      className="finstack-market-scenario-report flex flex-col gap-4 font-sans text-sm text-foreground"
      data-density={density}
      aria-busy={loading}
    >
      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {(
          [
            ["User operations", report.user_operations],
            ["Expanded operations", report.expanded_operations],
            ["Effects applied", report.operations_applied],
            [
              "Valuation date changed",
              report.changes.as_of_changed ? "yes" : "no",
            ],
          ] as const
        ).map(([term, value]) => (
          <div key={term} className="flex flex-col gap-0.5">
            <dt className="text-xs text-muted-foreground">{term}</dt>
            <dd className="finstack-numeric">{value}</dd>
          </div>
        ))}
      </dl>
      <FinstackTable
        caption="Applied shocks"
        data={indexed(report.applied_shocks ?? [])}
        columns={shockColumns}
        getRowId={(row) => String(row.index)}
        emptyState="No market or instrument shock applied"
        density={density}
      />
      {roll && (
        <section aria-label="Time roll" className="flex flex-col gap-2">
          <h3 className="text-sm font-semibold">Time roll</h3>
          <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <div className="flex flex-col gap-0.5">
              <dt className="text-xs text-muted-foreground">From</dt>
              <dd className="font-mono">{roll.old_date}</dd>
            </div>
            <div className="flex flex-col gap-0.5">
              <dt className="text-xs text-muted-foreground">To</dt>
              <dd className="font-mono">{roll.new_date}</dd>
            </div>
            <div className="flex flex-col gap-0.5">
              <dt className="text-xs text-muted-foreground">Calendar days</dt>
              <dd className="finstack-numeric">{roll.days}</dd>
            </div>
            <div className="flex flex-col gap-0.5">
              <dt className="text-xs text-muted-foreground">Total carry</dt>
              <dd className="flex flex-col">
                {Object.entries(roll.total_carry).map(([currency, money]) => (
                  <MoneyValue key={currency} value={money as Money} />
                ))}
              </dd>
            </div>
          </dl>
          <FinstackTable
            caption="Instrument carry"
            data={carry}
            columns={carryColumns}
            getRowId={(row) => `${row.id}:${row.money.currency}`}
            emptyState="No instrument carry returned"
            density={density}
          />
          {roll.failed_instruments.length > 0 && (
            <ul aria-label="Failed instruments" className="text-xs text-error">
              {roll.failed_instruments.map(([id, reason]) => (
                <li key={id}>
                  <span className="font-mono">{id}</span> {reason}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
      <FinstackTable
        caption="Warnings"
        data={indexed(report.warnings)}
        columns={warningColumns}
        getRowId={(row) => String(row.index)}
        emptyState="No warnings"
        density={density}
      />
      <StampBadge meta={report.meta} />
      <JsonViewer
        label="Application report"
        text={JSON.stringify(report, null, 2)}
        downloadName="scenario-report.json"
        density={density}
      />
    </div>
  );
}
