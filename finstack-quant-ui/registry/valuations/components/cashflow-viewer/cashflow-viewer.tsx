"use client";
import { useMemo } from "react";
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
import { readCashflows, type CashflowSchedule } from "./cashflow-data";

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
/** Native cashflow envelope as a table, with a host serialization of the supplied object. */
export function CashflowViewer({
  cashflows,
  loading = false,
  error: suppliedError,
  density = "compact",
}: {
  /** Native cashflow envelope object as returned by the facade; no worker or query provider is required. */
  cashflows?: CashflowSchedule | null;
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
          {schedule.flows.length ? (
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
          <p className="mt-2 text-xs text-muted-foreground">
            Amounts use each row’s currency. PV and total PV use reporting
            currency {schedule.currency}.
          </p>
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
