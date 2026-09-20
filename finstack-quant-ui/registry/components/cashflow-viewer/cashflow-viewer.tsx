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
import { formatMoney } from "@/lib/finstack/format/format";
import {
  useCashflows,
  type CashflowRequest,
} from "@/hooks/use-cashflows/use-cashflows";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import { readCashflows } from "./cashflow-data";

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
/** Native cashflow schedule with exact numeric tokens and an unchanged JSON export. */
export function CashflowViewer({
  request,
  density = "compact",
}: {
  request: CashflowRequest;
  density?: "compact" | "comfortable";
}) {
  const query = useCashflows(request);
  const presentation = useMemo(() => {
    if (!query.data) return {};
    try {
      return { schedule: readCashflows(query.data) };
    } catch (error) {
      return {
        error: `Cashflow table unavailable: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }, [query.data]);
  const schedule = presentation.schedule;
  const loading = query.isFetching || query.workerStatus === "starting";
  const error = query.error
    ? `Cashflows unavailable: ${query.error.message}`
    : presentation.error;
  const columns = diagnostics.filter(([key]) =>
    schedule?.flows.some((row) => row[key] !== undefined),
  );
  const hasExtra = schedule?.flows.some((row) =>
    extraDiagnostics.some(([key]) => row[key] !== undefined),
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
                        title={row.amount}
                      >
                        <span className="whitespace-nowrap">
                          {formatMoney({
                            amount: row.amount,
                            currency: row.currency,
                          })}
                        </span>
                      </TableCell>
                      {columns.map(([key]) => (
                        <TableCell
                          key={key}
                          className={`${cell} ${numeric} align-top print:hidden`}
                          title={row[key]}
                        >
                          {row[key] ?? "—"}
                        </TableCell>
                      ))}
                      <TableCell
                        className={`${cell} hidden align-top print:table-cell`}
                      >
                        <dl className="space-y-1">
                          {[...columns, ...extraDiagnostics].flatMap(
                            ([key, label]) =>
                              row[key] === undefined
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
                                        {row[key]}
                                      </dd>
                                    </div>,
                                  ],
                          )}
                        </dl>
                      </TableCell>
                      <TableCell
                        className={`${cell} ${numeric} align-top`}
                        title={row.pv}
                      >
                        <span className="whitespace-nowrap">
                          {formatMoney({
                            amount: row.pv,
                            currency: schedule.currency,
                          })}
                        </span>
                      </TableCell>
                      {hasExtra && (
                        <TableCell className={`${cell} align-top print:hidden`}>
                          <dl>
                            {extraDiagnostics.flatMap(([key, label]) =>
                              row[key] === undefined
                                ? []
                                : [
                                    <div
                                      key={key}
                                      className="flex justify-between gap-3"
                                    >
                                      <dt>{label}</dt>
                                      <dd className={numeric}>{row[key]}</dd>
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
                    <TableCell className={numeric} title={schedule.total_pv}>
                      <span className="whitespace-nowrap">
                        {formatMoney({
                          amount: schedule.total_pv,
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
                `Total PV · ${formatMoney({ amount: schedule.total_pv, currency: schedule.currency })}`}
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
          Original JSON
        </summary>
        <div className="mt-2">
          <JsonViewer
            label="Cashflow JSON"
            downloadName="cashflows.json"
            text={query.data}
            density={density}
          />
        </div>
      </details>
    </section>
  );
}
