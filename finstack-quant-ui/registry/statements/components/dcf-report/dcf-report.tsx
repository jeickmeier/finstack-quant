"use client";
import type { ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { MoneyValue as Money } from "finstack-quant-wasm";
import type { statements_analytics as wire } from "finstack-quant-wasm/types";
import type { DcfSensitivityResultWire } from "@/lib/finstack/generated/types/dcf_sensitivity_result";
import { formatExact } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";
import { TornadoChart } from "@/components/finstack/statements/components/tornado-chart/tornado-chart";
import {
  Pairs,
  ReportSection,
} from "@/components/finstack/shared/primitives/native-display/native-display";

/** Native statements_analytics CorporateValuationResult, as evaluateDcf returns it. */
export type CorporateValuationResultWire = wire.CorporateValuationResult;
type PeriodRow = wire.DcfPeriodRow;
export interface DcfReportProps {
  /** Native CorporateValuationResult; null while none is available. */
  result: CorporateValuationResultWire | null | undefined;
  /** Native DcfSensitivityResult; omitted hides the sensitivity section. */
  sensitivity?: DcfSensitivityResultWire | null;
  loading?: boolean;
  /** Native valuation error, shown as returned. */
  error?: string;
  sensitivityLoading?: boolean;
  /** Native sensitivity error, shown as returned. */
  sensitivityError?: string;
  /** Host note shown with the sensitivity (for example that it evaluates without the market). */
  sensitivityNote?: ReactNode;
  /** Display text of a returned number; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Presents a native DCF valuation as returned: enterprise and equity value
 * tiles, the enterprise-to-equity bridge fields, the per-period present values,
 * the terminal value and, when supplied, the native enterprise-value tornado.
 * Monetary amounts keep Rust's exact decimal text; nothing is summed,
 * discounted or rearranged here.
 */
export function DcfReport({
  result,
  sensitivity,
  loading,
  error,
  sensitivityLoading,
  sensitivityError,
  sensitivityNote,
  formatValue = formatExact,
  density = "compact",
}: DcfReportProps) {
  const money = (value: Money | null | undefined) => (
    <MoneyValue value={value ?? null} />
  );
  const number = (value: number | null | undefined) =>
    value == null ? "—" : formatValue(value);
  const currency = result?.enterprise_value.currency;
  const unit = (name: string) =>
    currency ? `${labelFor(name)} (${currency})` : labelFor(name);
  const periodColumns: ColumnDef<{}, PeriodRow, any>[] = [
    {
      id: "period_id",
      header: labelFor("period_id"),
      accessorFn: (row) => row.period_id,
      meta: { className: "font-mono" },
    },
    {
      id: "date",
      header: labelFor("date"),
      accessorFn: (row) => row.date,
      meta: { className: "font-mono" },
    },
    ...(
      [
        ["free_cash_flow", true],
        ["discount_years", false],
        ["discount_factor", false],
        ["present_value", true],
      ] as const
    ).map(([name, monetary]): ColumnDef<{}, PeriodRow, any> => ({
      id: name,
      header: monetary ? unit(name) : labelFor(name),
      accessorFn: (row) => row[name],
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => number(context.getValue() as number),
    })),
  ];
  const tile = (label: string, body: ReactNode) => (
    <section
      key={label}
      aria-label={label}
      className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
    >
      <h4 className="text-xs font-medium text-muted-foreground">{label}</h4>
      <p className="text-base">{body}</p>
    </section>
  );
  return (
    <section
      aria-label="DCF valuation"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !result ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Valuing…" : "No DCF valuation available."}
        </p>
      ) : (
        <>
          <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
            {tile(labelFor("enterprise_value"), money(result.enterprise_value))}
            {tile(labelFor("equity_value"), money(result.equity_value))}
            {result.equity_value_per_share != null &&
              tile(
                labelFor("equity_value_per_share"),
                <span className="finstack-numeric">
                  {number(result.equity_value_per_share)}
                </span>,
              )}
          </div>
          <ReportSection title="Enterprise value to equity value">
            <Pairs
              label="Returned bridge"
              rows={[
                [labelFor("enterprise_value"), money(result.enterprise_value)],
                [labelFor("net_debt"), money(result.net_debt)],
                [
                  labelFor("valuation_discount"),
                  money(result.valuation_discount),
                ],
                [labelFor("equity_value"), money(result.equity_value)],
                ...(result.diluted_shares != null
                  ? [
                      [
                        labelFor("diluted_shares"),
                        number(result.diluted_shares),
                      ] as const,
                    ]
                  : []),
              ]}
            />
            {result.equity_bridge && (
              <Pairs
                label="Equity bridge inputs"
                rows={Object.entries(result.equity_bridge).flatMap(
                  ([name, value]): (readonly [string, ReactNode])[] =>
                    name === "other_adjustments" && Array.isArray(value)
                      ? (value as [string, number][]).map(
                          ([adjustment, amount]) =>
                            [adjustment, number(amount)] as const,
                        )
                      : [[unit(name), number(value as number)] as const],
                )}
              />
            )}
            <p className="text-xs text-muted-foreground">
              Fields in the order Rust returns them; the bridge inputs are the
              model currency amounts Rust used.
            </p>
          </ReportSection>
          <ReportSection title="Explicit forecast periods">
            <FinstackTable
              data={result.periods ?? []}
              columns={periodColumns}
              getRowId={(row) => row.period_id}
              caption="Discounted unlevered free cash flow by period"
              captionVisibility="sr-only"
              density={density}
              emptyState="No periods returned"
            />
            <Pairs
              label="Explicit present value"
              rows={[[labelFor("pv_explicit"), money(result.pv_explicit)]]}
            />
          </ReportSection>
          <ReportSection title="Terminal value">
            <Pairs
              label="Returned terminal value"
              rows={[
                [labelFor("terminal_value"), money(result.terminal_value)],
                [
                  labelFor("terminal_discount_years"),
                  number(result.terminal_discount_years),
                ],
                [
                  labelFor("terminal_value_pv"),
                  money(result.terminal_value_pv),
                ],
                [`${labelFor("wacc")} (decimal)`, number(result.wacc)],
              ]}
            />
          </ReportSection>
        </>
      )}
      {sensitivity !== undefined &&
        (sensitivity || sensitivityError || sensitivityLoading) && (
          <ReportSection title="Enterprise value sensitivity">
            {sensitivityNote && (
              <p className="text-xs text-muted-foreground">{sensitivityNote}</p>
            )}
            {sensitivityError ? (
              <p role="alert" className="text-sm text-error">
                {sensitivityError}
              </p>
            ) : !sensitivity ? (
              <p role="status" className="text-sm text-muted-foreground">
                Ranking assumptions…
              </p>
            ) : (
              <>
                <Pairs
                  label="Effective shocks"
                  rows={[
                    [
                      labelFor("baseline_enterprise_value"),
                      money(sensitivity.baseline_enterprise_value),
                    ],
                    [
                      `${labelFor("wacc_down")} (decimal)`,
                      `${number(sensitivity.wacc_down)}${sensitivity.wacc_down_clamped ? " (clamped)" : ""}`,
                    ],
                    [
                      `${labelFor("terminal_growth_up")} (decimal)`,
                      `${number(sensitivity.terminal_growth_up)}${sensitivity.terminal_growth_up_clamped ? " (clamped)" : ""}`,
                    ],
                  ]}
                />
                <TornadoChart
                  entries={sensitivity.entries}
                  valueLabel={`Enterprise value change (${sensitivity.baseline_enterprise_value.currency})`}
                  caption="Enterprise value change by assumption"
                  formatValue={formatValue}
                  density={density}
                />
                <p className="text-xs text-muted-foreground">
                  Changes against the baseline enterprise value, as Rust
                  returned them, in Rust's order (largest swing first).
                </p>
              </>
            )}
          </ReportSection>
        )}
    </section>
  );
}
