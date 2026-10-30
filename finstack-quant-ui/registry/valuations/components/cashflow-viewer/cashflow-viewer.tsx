"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
  TableFooter,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { formatRaw, formatRawMoney } from "@/lib/finstack/format/format";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  readCashflows,
  type CashflowLadder,
  type CashflowSchedule,
} from "./cashflow-data";

const diagnostics = [
  ["rate", "Rate", "Projected or contractual rate, as supplied"],
  [
    "year_fraction",
    "Year fraction",
    "From as-of to payment under the discount curve day count",
  ],
  ["discount_factor", "DF", "Discount factor from as-of to payment"],
] as const;
const extraDiagnostics = [
  ["survival_probability", "Survival"],
  ["conditional_default_prob", "Interval default probability"],
  ["inflation_index_ratio", "Inflation ratio"],
  ["prepayment_smm", "Prepayment SMM"],
  ["beginning_balance", "Beginning balance"],
  ["ending_balance", "Ending balance"],
] as const;
const views = [
  { value: "schedule", label: "Schedule" },
  { value: "ladder", label: "Annual ladder" },
] as const;
type LadderRow = CashflowLadder["rows"][number];
/** Supplied native ladder rows in a row currency; values are formatted, never summed. */
function ladderColumns(
  currency: CashflowLadder["currency"],
): ColumnDef<{}, LadderRow, any>[] {
  const money = (
    id: "non_principal" | "principal" | "pv",
    header: string,
    title?: string,
  ): ColumnDef<{}, LadderRow, any> => ({
    id,
    header: () => <span title={title}>{header}</span>,
    accessorFn: (row) => row[id],
    meta: { className: "text-right finstack-numeric font-sans" },
    cell: (context) => (
      <span className="whitespace-nowrap" title={String(context.getValue())}>
        {formatRawMoney({ amount: String(context.getValue()), currency })}
      </span>
    ),
  });
  return [
    {
      id: "year",
      header: "Year",
      accessorFn: (row) => row.year,
      meta: { className: "font-mono", rowHeader: true },
    },
    money("non_principal", "Non-principal"),
    money("principal", "Principal"),
    money(
      "pv",
      "Native PV",
      "Row-currency present value before FX conversion and any scenario price shock",
    ),
  ];
}
/** Native cashflow envelope as a table, with an optional native annual ladder and a host serialization of the supplied object. */
export function CashflowViewer({
  cashflows,
  ladder,
  ladderError,
  loading = false,
  error: suppliedError,
  density = "compact",
}: {
  /** Native cashflow envelope object as returned by the facade; no worker or query provider is required. */
  cashflows?: CashflowSchedule | null;
  /** Native calendar-year ladder per row currency of the same envelope (useCashflowLadder); supplying it, even as null while pending, adds the Annual ladder view. */
  ladder?: readonly CashflowLadder[] | null;
  /** Native ladder error supplied by the owner of the ladder request. */
  ladderError?: string | null;
  loading?: boolean;
  /** Native query error supplied by the owner of the request. */
  error?: string | null;
  density?: "compact" | "comfortable";
}) {
  const presentation = useMemo(() => {
    if (!cashflows) return {};
    try {
      return { schedule: readCashflows(cashflows) };
    } catch (error) {
      return {
        error: `Cashflow table unavailable: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }, [cashflows]);
  // Host serialization of the supplied object for copy and download; not native text.
  const text = useMemo(() => {
    if (!cashflows) return null;
    try {
      return serializeHost(cashflows);
    } catch {
      return null;
    }
  }, [cashflows]);
  const [view, setView] = useState<"schedule" | "ladder">("schedule");
  const hasLadder = ladder !== undefined || Boolean(ladderError);
  const showLadder = hasLadder && view === "ladder";
  const schedule = presentation.schedule;
  const error = suppliedError
    ? `Cashflows unavailable: ${suppliedError}`
    : presentation.error;
  const columns = diagnostics.filter(([key]) =>
    schedule?.flows.some((row) => row[key] != null),
  );
  const hasExtra = schedule?.flows.some((row) =>
    extraDiagnostics.some(([key]) => row[key] != null),
  );
  const numeric = "text-right finstack-numeric font-sans";
  const cell = density === "compact" ? "py-1.5" : "py-3";
  return (
    <section
      aria-label="Cashflows"
      data-density={density}
      aria-busy={loading}
      className="min-w-0 max-w-full font-sans text-foreground"
    >
      <h3 className="hidden text-sm font-semibold print:block">Cashflows</h3>
      {loading && (
        <p role="status" className="text-sm text-muted-foreground">
          Loading cashflows…
        </p>
      )}
      {error && (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      )}
      {schedule && !error && (
        <>
          <div className="mb-3 flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
            <span>
              {schedule.flows.length} cashflows ·{" "}
              <span className="font-mono">{schedule.instrument_id}</span>
            </span>
            <span>
              As of <span className="font-mono">{schedule.as_of}</span> ·{" "}
              {schedule.model}
            </span>
          </div>
          {hasLadder && (
            <div className="mb-3 print:hidden">
              <EnumField
                label="Cashflow view"
                orientation="horizontal"
                layout="radio"
                value={view}
                onValueChange={(next) =>
                  setView(next === "ladder" ? "ladder" : "schedule")
                }
                options={views}
              />
            </div>
          )}
          {showLadder ? (
            <div aria-label="Annual ladder" role="group" className="space-y-4">
              {ladderError ? (
                <p role="alert" className="text-sm text-error">
                  Annual ladder unavailable: {ladderError}
                </p>
              ) : !ladder ? (
                <p role="status" className="text-sm text-muted-foreground">
                  Loading annual ladder…
                </p>
              ) : ladder.length ? (
                ladder.map((entry) => (
                  <FinstackTable
                    key={entry.currency}
                    data={[...entry.rows]}
                    columns={ladderColumns(entry.currency)}
                    getRowId={(row) => String(row.year)}
                    caption={`Annual ladder · ${entry.currency}`}
                    density={density}
                    emptyState="No ladder rows returned."
                  />
                ))
              ) : (
                <p className="py-4 text-sm text-muted-foreground">
                  No ladder rows returned.
                </p>
              )}
              <p className="text-xs text-muted-foreground">
                Rust calendar-year ladder per row currency. Amounts and native
                PV are in that currency, before FX conversion and any scenario
                price shock; no FX conversion or totals across currencies.
              </p>
            </div>
          ) : schedule.flows.length ? (
            <div className="max-w-full [&_[data-slot=table-container]]:print:overflow-visible">
              <Table
                aria-label="Cashflow schedule"
                tabIndex={0}
                className="text-xs focus-visible:outline-2 focus-visible:outline-ring print:text-xs"
              >
                <TableHeader>
                  <TableRow>
                    <TableHead scope="col">Payment date</TableHead>
                    <TableHead scope="col" className="print:hidden">
                      Kind
                    </TableHead>
                    <TableHead scope="col" className="text-right">
                      Amount
                    </TableHead>
                    {columns.map(([key, label, title]) => (
                      <TableHead
                        key={key}
                        scope="col"
                        className="text-right print:hidden"
                        title={title}
                      >
                        {label}
                      </TableHead>
                    ))}
                    <TableHead scope="col" className="hidden print:table-cell">
                      Pricing factors
                    </TableHead>
                    <TableHead scope="col" className="text-right">
                      PV · {schedule.currency}
                    </TableHead>
                    {hasExtra && (
                      <TableHead scope="col" className="print:hidden">
                        Diagnostics
                      </TableHead>
                    )}
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {schedule.flows.map((row, index) => (
                    <TableRow key={index}>
                      <TableCell className={`${cell} font-mono align-top`}>
                        <span className="whitespace-nowrap">{row.date}</span>
                        <span className="hidden whitespace-nowrap font-sans text-muted-foreground print:block">
                          {row.kind}
                        </span>
                      </TableCell>
                      <TableCell className={`${cell} align-top print:hidden`}>
                        <span>{row.kind}</span>
                      </TableCell>
                      <TableCell
                        className={`${cell} ${numeric} align-top`}
                        title={String(row.amount)}
                      >
                        <span className="whitespace-nowrap">
                          {formatRawMoney({
                            amount: String(row.amount),
                            currency: row.currency,
                          })}
                        </span>
                      </TableCell>
                      {columns.map(([key]) => (
                        <TableCell
                          key={key}
                          className={`${cell} ${numeric} align-top print:hidden`}
                          title={
                            row[key] == null ? undefined : String(row[key])
                          }
                        >
                          {formatRaw(row[key])}
                        </TableCell>
                      ))}
                      <TableCell
                        className={`${cell} hidden align-top print:table-cell`}
                      >
                        <dl className="space-y-1">
                          {[...columns, ...extraDiagnostics].flatMap(
                            ([key, label]) =>
                              row[key] == null
                                ? []
                                : [
                                    <div
                                      key={key}
                                      className="flex justify-between gap-2"
                                    >
                                      <dt className="text-muted-foreground">
                                        {label}
                                      </dt>
                                      <dd className="finstack-numeric whitespace-nowrap font-sans">
                                        {formatRaw(row[key])}
                                      </dd>
                                    </div>,
                                  ],
                          )}
                        </dl>
                      </TableCell>
                      <TableCell
                        className={`${cell} ${numeric} align-top`}
                        title={String(row.pv)}
                      >
                        <span className="whitespace-nowrap">
                          {formatRawMoney({
                            amount: String(row.pv),
                            currency: schedule.currency,
                          })}
                        </span>
                      </TableCell>
                      {hasExtra && (
                        <TableCell className={`${cell} align-top print:hidden`}>
                          <dl>
                            {extraDiagnostics.flatMap(([key, label]) =>
                              row[key] == null
                                ? []
                                : [
                                    <div
                                      key={key}
                                      className="flex justify-between gap-3"
                                    >
                                      <dt>{label}</dt>
                                      <dd className={numeric}>
                                        {formatRaw(row[key])}
                                      </dd>
                                    </div>,
                                  ],
                            )}
                          </dl>
                        </TableCell>
                      )}
                    </TableRow>
                  ))}
                </TableBody>
                <TableFooter>
                  <TableRow>
                    <TableCell
                      colSpan={3 + columns.length}
                      className="text-right print:hidden"
                    >
                      Total PV · {schedule.currency}
                    </TableCell>
                    <TableCell
                      colSpan={3}
                      className="hidden text-right print:table-cell"
                    >
                      Total PV · {schedule.currency}
                    </TableCell>
                    <TableCell
                      className={numeric}
                      title={String(schedule.total_pv)}
                    >
                      <span className="whitespace-nowrap">
                        {formatRawMoney({
                          amount: String(schedule.total_pv),
                          currency: schedule.currency,
                        })}
                      </span>
                    </TableCell>
                    {hasExtra && <TableCell className="print:hidden" />}
                  </TableRow>
                </TableFooter>
              </Table>
            </div>
          ) : (
            <p className="py-4 text-sm text-muted-foreground">
              No cashflows returned.
            </p>
          )}
          <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-border pt-3 text-xs">
            <Badge variant="secondary">
              {schedule.reconciles_with_base_value
                ? "Reconciled to base value"
                : "Not reconciled to base value"}
            </Badge>
            <span>
              {!schedule.flows.length &&
                `Total PV · ${formatRawMoney({ amount: String(schedule.total_pv), currency: schedule.currency })}`}
            </span>
          </div>
          {!showLadder && (
            <p className="mt-2 text-xs text-muted-foreground">
              Amounts use each row’s currency. PV and total PV use reporting
              currency {schedule.currency}.
            </p>
          )}
        </>
      )}
      {!loading && !error && !schedule && (
        <p className="text-sm text-muted-foreground">No cashflows supplied.</p>
      )}
      <details className="mt-4 text-xs print:hidden">
        <summary className="cursor-pointer text-muted-foreground focus-visible:outline-2 focus-visible:outline-ring">
          Envelope JSON
        </summary>
        <div className="mt-2">
          <JsonViewer
            label="Cashflow JSON"
            downloadName="cashflows.json"
            text={text}
            density={density}
          />
        </div>
      </details>
    </section>
  );
}
