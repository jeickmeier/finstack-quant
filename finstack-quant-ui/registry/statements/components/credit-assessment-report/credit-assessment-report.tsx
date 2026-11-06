"use client";
import { useMemo, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { colorLegend, defineChart, lineY } from "@tanstack/charts";
import { scalePoint } from "@tanstack/charts/scales/point";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { CreditAssessmentWire } from "@/lib/finstack/generated/types/credit_assessment";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { formatExact } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";

type SeriesPoint = CreditAssessmentWire["series"][number];
/** The CreditAssessment fields shown per period, in Rust's field order. */
const measures = [
  "leverage_ratio",
  "interest_coverage",
  "free_cash_flow",
] as const;
type Measure = (typeof measures)[number];
/** The two ratio measures share one chart; free cash flow is monetary and stays in the table. */
const ratios = ["leverage_ratio", "interest_coverage"] as const;
interface RatioPoint {
  measure: string;
  period: string;
  value: number;
}
export interface CreditAssessmentReportProps {
  /** Native statements_analytics CreditAssessment; null while none is available. */
  assessment: CreditAssessmentWire | null | undefined;
  /** Native P&L summary TableEnvelope (statements_analytics.plSummaryReport); omitted hides the table. */
  plSummary?: TableEnvelopeWire | null;
  /** Model currency of the free cash flow, as the host's model declares it; omitted shows no unit. */
  currency?: string | null;
  /** Display text of a returned value; defaults to every supplied digit. */
  formatValue?: (measure: Measure, value: number) => string;
  loading?: boolean;
  /** Native assessment error, shown as returned. */
  error?: string;
  plSummaryLoading?: boolean;
  /** Native P&L summary error, shown as returned. */
  plSummaryError?: string;
  density?: "compact" | "comfortable";
}
/**
 * Presents a native credit assessment as returned: leverage, interest
 * coverage and free cash flow at the assessed period, the per-period series
 * (ratios charted, null points left out and listed) and, when supplied, the
 * native P&L summary table. Nothing is calculated here.
 */
export function CreditAssessmentReport({
  assessment,
  plSummary,
  currency,
  formatValue = (_measure, value) => formatExact(value),
  loading,
  error,
  plSummaryLoading,
  plSummaryError,
  density = "compact",
}: CreditAssessmentReportProps) {
  const label = (measure: Measure) =>
    measure === "free_cash_flow" && currency
      ? `${labelFor(measure)} (${currency})`
      : labelFor(measure);
  const shown = (measure: Measure, value: number | null | undefined) =>
    value == null ? "—" : formatValue(measure, value);
  const plotted = useMemo(() => {
    const points: RatioPoint[] = [];
    const omitted: { measure: string; periods: string[] }[] = [];
    for (const measure of ratios) {
      const missing: string[] = [];
      for (const point of assessment?.series ?? []) {
        const value = point[measure];
        if (typeof value === "number" && Number.isFinite(value))
          points.push({
            measure: labelFor(measure),
            period: point.period,
            value,
          });
        else missing.push(point.period);
      }
      if (missing.length) omitted.push({ measure, periods: missing });
    }
    return { points, omitted };
  }, [assessment]);
  const chart = useMemo(() => {
    const periods = (assessment?.series ?? []).map((point) => point.period);
    return defineChart({
      marks: [
        lineY(plotted.points, {
          x: (p: RatioPoint) => p.period,
          y: (p: RatioPoint) => p.value,
          z: (p: RatioPoint) => p.measure,
          key: (p: RatioPoint) => `${p.measure}|${p.period}`,
          id: "credit-ratios",
        }),
      ],
      scales: {
        x: {
          scale: () => scalePoint<string>().domain(periods).padding(0.5),
          axis: { label: "Period (returned order)" },
        },
        y: { scale: scaleLinear, nice: true, axis: { label: "Multiple (x)" } },
      },
      color: { legend: colorLegend() },
      tooltip: {
        use: tooltip,
        content: (marks) => ({
          rows: marks.map((m) => ({
            label: `${m.datum.measure} · ${m.datum.period}`,
            value: formatExact(m.datum.value),
          })),
        }),
      },
    });
  }, [assessment, plotted]);
  const columns: ColumnDef<{}, SeriesPoint, any>[] = [
    {
      id: "period",
      header: labelFor("period"),
      accessorFn: (row) => row.period,
      meta: { className: "font-mono" },
    },
    ...measures.map((measure): ColumnDef<{}, SeriesPoint, any> => ({
      id: measure,
      header: label(measure),
      accessorFn: (row) => row[measure],
      meta: { className: "text-right finstack-numeric" },
      cell: (context) =>
        shown(measure, context.getValue() as number | null | undefined),
    })),
  ];
  const tile = (measure: Measure, body: ReactNode) => (
    <section
      key={measure}
      aria-label={label(measure)}
      className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
    >
      <h4 className="text-xs font-medium text-muted-foreground">
        {label(measure)}
      </h4>
      {body}
    </section>
  );
  return (
    <section
      aria-label="Credit assessment"
      data-density={density}
      className="@container min-w-0 space-y-4 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !assessment ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Assessing credit…" : "No credit assessment available."}
        </p>
      ) : (
        <>
          <h3 className="text-sm font-medium">
            Credit assessment at{" "}
            <span className="font-mono">{assessment.period}</span>
          </h3>
          <div className="grid grid-cols-1 gap-2 @lg:grid-cols-3">
            {measures.map((measure) =>
              tile(
                measure,
                <p className="finstack-numeric text-base">
                  {shown(measure, assessment[measure])}
                </p>,
              ),
            )}
          </div>
          {plotted.points.length > 0 && (
            <figure className="min-w-0">
              <FinstackChart
                definition={chart}
                ariaLabel="Leverage ratio and interest coverage by period"
                height={240}
              />
            </figure>
          )}
          {plotted.omitted.length > 0 && (
            <p className="text-xs text-muted-foreground">
              Not plotted, returned without a value:{" "}
              {plotted.omitted
                .map(
                  ({ measure, periods }) =>
                    `${labelFor(measure)} ${periods.join(", ")}`,
                )
                .join("; ")}
              .
            </p>
          )}
          <FinstackTable
            data={assessment.series}
            columns={columns}
            getRowId={(row) => row.period}
            caption="Credit assessment by period"
            captionVisibility="sr-only"
            density={density}
            emptyState="No periods returned"
          />
          <p className="text-xs text-muted-foreground">
            Values as Rust returned them; a dash is a period Rust left without a
            value (for example before a full trailing year).
          </p>
        </>
      )}
      {plSummary !== undefined &&
        (plSummaryError || plSummary || plSummaryLoading) && (
          <section aria-label="P&L summary" className="min-w-0 space-y-2">
            <h3 className="text-sm font-medium">P&L summary</h3>
            {plSummary ? (
              <TableEnvelope
                envelope={plSummary}
                caption="P&L summary by line item and period"
                captionVisibility="sr-only"
                error={plSummaryError}
                density={density}
              />
            ) : plSummaryError ? (
              <p role="alert" className="text-sm text-error">
                {plSummaryError}
              </p>
            ) : (
              <p role="status" className="text-sm text-muted-foreground">
                Building the P&L summary…
              </p>
            )}
          </section>
        )}
    </section>
  );
}
