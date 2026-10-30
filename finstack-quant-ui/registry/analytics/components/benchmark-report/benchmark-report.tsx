"use client";
import {
  datedPoints,
  ReportSection,
  type DatedPoint,
} from "@/components/finstack/analytics/lib/analytics-report-presentation";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { areaY, defineChart, lineY } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleUtc } from "d3-scale";
import type {
  BetaResult,
  GreeksResult,
  MultiFactorResult,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** One ticker's native relative series, each aligned with that ticker's own active dates. */
export interface BenchmarkTickerData {
  activeDates: readonly string[];
  outperformance: readonly number[];
  drawdownDifference: readonly number[];
  rollingGreeks: {
    dates: readonly string[];
    alphas: readonly number[];
    betas: readonly number[];
  };
  /** The native regression, or its native error; null without a factor panel. */
  multiFactor:
    | { ok: true; value: MultiFactorResult }
    | { ok: false; error: { message: string } }
    | null;
}
/** The native analytics.Performance results the worker's benchmarkReport returns. */
export interface BenchmarkReportData {
  tickerNames: readonly string[];
  benchmarkIdx: number;
  frequency: string;
  activeDates: readonly string[];
  /** Factor labels of the regression, else null. */
  factorNames: readonly string[] | null;
  greeks: readonly GreeksResult[];
  beta: readonly BetaResult[];
  trackingError: readonly number[];
  informationRatio: readonly number[];
  treynor: readonly number[];
  mSquared: readonly number[];
  upCapture: readonly number[];
  downCapture: readonly number[];
  captureRatio: readonly number[];
  battingAverage: readonly number[];
  tickers: readonly BenchmarkTickerData[];
}
export interface BenchmarkReportProps {
  /** The returned report; null while none is available. */
  report: BenchmarkReportData | null;
  loading?: boolean;
  /** The native error message, shown instead of a report. */
  error?: string;
  /** Fund shown first; omitted is the first ticker that is not the benchmark. */
  initialTicker?: number;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
interface Row {
  measure: string;
  value: number | null | undefined;
  /** An integer count (Rust usize), shown as returned rather than through the host format. */
  count?: boolean;
}
function datedChart(
  points: readonly DatedPoint[],
  mark: "line" | "area",
  valueLabel: string,
  format: (value: number) => string,
) {
  const channels = {
    x: (p: DatedPoint) => p.date,
    y: (p: DatedPoint) => p.value,
    key: (p: DatedPoint) => `${p.series}|${p.iso}`,
  };
  return defineChart({
    marks: [
      mark === "area"
        ? areaY(points, { ...channels, id: "relative-area" })
        : lineY(points, { ...channels, id: "rolling-line" }),
    ],
    scales: {
      x: { scale: scaleUtc, axis: { label: "Date" } },
      y: { scale: scaleLinear, nice: true, axis: { label: valueLabel } },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: `${m.datum.series} · ${m.datum.iso}`,
          value: format(m.datum.value),
        })),
      }),
    },
  });
}
/** A two-column measure table; finite numbers use the host format, anything else is shown as returned. */
function MeasureTable({
  caption,
  rows,
  format,
  density,
}: {
  caption: string;
  rows: Row[];
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const columns = useMemo<ColumnDef<{}, Row, any>[]>(
    () => [
      {
        id: "measure",
        header: "Measure",
        accessorFn: (row) => row.measure,
        meta: { className: "text-left" },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
        cell: ({ getValue, row }) => {
          const value = getValue() as number | null | undefined;
          return typeof value === "number" &&
            Number.isFinite(value) &&
            !row.original.count
            ? format(value)
            : formatRaw(value);
        },
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [format],
  );
  return (
    <FinstackTable
      data={rows}
      columns={columns}
      getRowId={(row) => row.measure}
      caption={caption}
      captionVisibility="sr-only"
      density={density}
      emptyState="No measures returned"
    />
  );
}
/** Rows of a returned object, in the order listed, labelled by field name; `n_obs` is Rust's integer observation count. */
const fields = <T,>(
  value: T | undefined,
  keys: readonly (keyof T & string)[],
): Row[] =>
  keys.map((key) => ({
    measure: labelFor(key),
    value: value?.[key] as number | undefined,
    count: key === "n_obs",
  }));
/**
 * Presents one native analytics.Performance benchmark report: the regression,
 * beta estimate and relative measures of the selected fund, its outperformance
 * and drawdown difference, rolling alpha and beta and, when a factor panel was
 * supplied, its multi-factor regression. Every value is shown as Rust returned
 * it (decimal fractions and ratios, no units inferred); nothing is computed.
 */
export function BenchmarkReport({
  report,
  loading,
  error,
  ...props
}: BenchmarkReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Calculating benchmark measures…
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No benchmark report
      </p>
    );
  return <ReportView report={report} {...props} />;
}
function ReportView({
  report,
  initialTicker,
  formatValue = String,
  density = "compact",
}: Omit<BenchmarkReportProps, "report" | "loading" | "error"> & {
  report: BenchmarkReportData;
}) {
  const [selected, setSelected] = useState(() => {
    if (initialTicker !== undefined) return initialTicker;
    const fund = report.tickerNames.findIndex(
      (_, index) => index !== report.benchmarkIdx,
    );
    return fund === -1 ? 0 : fund;
  });
  const ticker = report.tickerNames[selected] ?? "";
  const benchmark = report.tickerNames[report.benchmarkIdx] ?? "";
  const series = report.tickers[selected];
  const charts = useMemo(() => {
    if (!series) return null;
    return {
      outperformance: datedChart(
        datedPoints(ticker, series.activeDates, series.outperformance),
        "area",
        "Cumulative outperformance (decimal)",
        formatValue,
      ),
      drawdown: datedChart(
        datedPoints(ticker, series.activeDates, series.drawdownDifference),
        "area",
        "Drawdown difference (decimal)",
        formatValue,
      ),
      alpha: datedChart(
        datedPoints(
          ticker,
          series.rollingGreeks.dates,
          series.rollingGreeks.alphas,
        ),
        "line",
        "Rolling alpha (annualized decimal)",
        formatValue,
      ),
      beta: datedChart(
        datedPoints(
          ticker,
          series.rollingGreeks.dates,
          series.rollingGreeks.betas,
        ),
        "line",
        "Rolling beta",
        formatValue,
      ),
    };
  }, [series, ticker, formatValue]);
  const relative: Row[] = (
    [
      ["tracking_error", report.trackingError],
      ["information_ratio", report.informationRatio],
      ["treynor", report.treynor],
      ["m_squared", report.mSquared],
      ["up_capture", report.upCapture],
      ["down_capture", report.downCapture],
      ["capture_ratio", report.captureRatio],
      ["batting_average", report.battingAverage],
    ] as const
  ).map(([key, values]) => ({
    measure: labelFor(key),
    value: values[selected],
  }));
  const multiFactor = series?.multiFactor ?? null;
  return (
    <section
      aria-label="Benchmark report"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-end gap-4">
        <div className="min-w-48">
          <EnumField
            label="Fund"
            value={String(selected)}
            onValueChange={(value) => setSelected(Number(value))}
            options={report.tickerNames.map((name, index) => ({
              value: String(index),
              label:
                index === report.benchmarkIdx ? `${name} (benchmark)` : name,
            }))}
            layout="select"
          />
        </div>
        <p className="text-muted-foreground">
          Against {benchmark} · {labelFor(report.frequency)} observations ·{" "}
          {report.activeDates[0] ?? "—"} to {report.activeDates.at(-1) ?? "—"}
        </p>
      </div>
      <div className="grid gap-6 @3xl:grid-cols-3">
        <ReportSection title={`${ticker} regression on ${benchmark}`}>
          <MeasureTable
            caption={`${ticker} regression on ${benchmark}`}
            rows={fields(report.greeks[selected], [
              "alpha",
              "beta",
              "r_squared",
              "adjusted_r_squared",
              "n_obs",
            ])}
            format={formatValue}
            density={density}
          />
        </ReportSection>
        <ReportSection title={`${ticker} beta estimate`}>
          <MeasureTable
            caption={`${ticker} beta estimate`}
            rows={fields(report.beta[selected], [
              "beta",
              "std_err",
              "ci_lower",
              "ci_upper",
              "confidence_level",
              "n_obs",
            ])}
            format={formatValue}
            density={density}
          />
        </ReportSection>
        <ReportSection title={`${ticker} relative measures`}>
          <MeasureTable
            caption={`${ticker} relative measures`}
            rows={relative}
            format={formatValue}
            density={density}
          />
        </ReportSection>
      </div>
      <p className="text-xs text-muted-foreground">
        Decimal fractions and ratios as Rust returned them; alphas are
        annualized.
      </p>
      {charts && series ? (
        <div className="grid gap-6 @3xl:grid-cols-2">
          <ReportSection title={`${ticker} cumulative outperformance`}>
            <FinstackChart
              definition={charts.outperformance}
              ariaLabel={`${ticker} cumulative outperformance`}
              height={240}
            />
          </ReportSection>
          <ReportSection title={`${ticker} drawdown difference`}>
            <FinstackChart
              definition={charts.drawdown}
              ariaLabel={`${ticker} drawdown difference`}
              height={240}
            />
          </ReportSection>
          <ReportSection title={`${ticker} rolling alpha`}>
            <FinstackChart
              definition={charts.alpha}
              ariaLabel={`${ticker} rolling alpha`}
              height={240}
            />
          </ReportSection>
          <ReportSection title={`${ticker} rolling beta`}>
            <FinstackChart
              definition={charts.beta}
              ariaLabel={`${ticker} rolling beta`}
              height={240}
            />
          </ReportSection>
        </div>
      ) : (
        <p role="status">No ticker series returned</p>
      )}
      {multiFactor ? (
        <ReportSection title={`${ticker} multi-factor regression`}>
          {multiFactor.ok ? (
            <MeasureTable
              caption={`${ticker} multi-factor regression`}
              rows={[
                { measure: labelFor("alpha"), value: multiFactor.value.alpha },
                ...multiFactor.value.betas.map((value, index) => ({
                  measure: `Beta · ${report.factorNames?.[index] ?? index + 1}`,
                  value,
                })),
                ...fields(multiFactor.value, [
                  "r_squared",
                  "adjusted_r_squared",
                  "residual_vol",
                  "n_obs",
                ]),
              ]}
              format={formatValue}
              density={density}
            />
          ) : (
            <p role="alert" className="text-error">
              {multiFactor.error.message}
            </p>
          )}
        </ReportSection>
      ) : null}
    </section>
  );
}
