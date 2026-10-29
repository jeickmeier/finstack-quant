"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { areaY, barY, colorLegend, defineChart, lineY } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleUtc } from "d3-scale";
import type {
  DatedSeries,
  DrawdownEpisode,
  LookbackReturns,
  PeriodicReturn,
} from "finstack-quant-wasm";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  FinstackChart,
  heatmap,
} from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import type { HeatmapColors } from "@/components/finstack/shared/chart/finstack-chart/heatmap";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";

/** One ticker's native series, each aligned with that ticker's own active dates. */
export interface PerformanceTickerData {
  activeDates: readonly string[];
  cumulativeReturns: readonly number[];
  drawdowns: readonly number[];
  drawdownEpisodes: readonly DrawdownEpisode[];
  rollingSharpe: Omit<DatedSeries, "values"> & { values: readonly number[] };
  rollingVolatility: Omit<DatedSeries, "values"> & {
    values: readonly number[];
  };
}
/** The native analytics.Performance results the worker's performanceReport returns. */
export interface PerformanceReportData {
  tickerNames: readonly string[];
  /** Index of the named benchmark; null when none was named. */
  benchmarkIdx: number | null;
  frequency: string;
  activeDates: readonly string[];
  /** Performance.summary() as returned. */
  summary: TableEnvelopeWire;
  tickers: readonly PerformanceTickerData[];
  monthly: readonly (readonly PeriodicReturn[])[];
  annual: readonly (readonly PeriodicReturn[])[];
  lookback: {
    refDate: string;
    returns: LookbackReturns;
    fiscalYearStart: { month: number; day: number } | null;
  };
}
export interface PerformanceReportProps {
  /** The returned report; null while none is available. */
  report: PerformanceReportData | null;
  loading?: boolean;
  /** The native error message, shown instead of a report. */
  error?: string;
  /** Ticker shown first in the per-ticker sections; omitted is the first ticker. */
  initialTicker?: number;
  /**
   * Explicit monthly-heatmap colour domain. Omitted derives the returned
   * values' extent (centred on zero when it straddles zero). A domain that
   * excludes a returned value is reported instead of drawn.
   */
  monthlyDomain?: HeatmapColors;
  /** Display text of a finite returned number in the tiles, heatmap and tooltips; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
interface DatedPoint {
  ticker: string;
  iso: string;
  date: Date;
  value: number;
}
const lookbackKeys = ["mtd", "qtd", "ytd", "fytd"] as const;
const utc = (iso: string) => new Date(`${iso}T00:00:00Z`);
/** Finite native values against their dates; non-finite entries are omitted, never replaced. */
function datedPoints(
  ticker: string,
  dates: readonly string[],
  values: readonly number[],
): DatedPoint[] {
  return dates.flatMap((iso, index) => {
    const value = values[index];
    return typeof value === "number" && Number.isFinite(value)
      ? [{ ticker, iso, date: utc(iso), value }]
      : [];
  });
}
/** Host display text for finite numbers; anything else is shown as is, never as zero. */
function display(
  value: number | null | undefined,
  format: (value: number) => string,
) {
  return typeof value === "number" && Number.isFinite(value)
    ? format(value)
    : formatRaw(value);
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
    key: (p: DatedPoint) => `${p.ticker}|${p.iso}`,
  };
  return defineChart({
    marks: [
      mark === "area"
        ? areaY(points, { ...channels, id: "dated-area" })
        : lineY(points, {
            ...channels,
            z: (p: DatedPoint) => p.ticker,
            id: "dated-line",
          }),
    ],
    scales: {
      x: { scale: scaleUtc, axis: { label: "Date" } },
      y: { scale: scaleLinear, nice: true, axis: { label: valueLabel } },
    },
    ...(mark === "line" ? { color: { legend: colorLegend() } } : {}),
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: `${m.datum.ticker} · ${m.datum.iso}`,
          value: format(m.datum.value),
        })),
      }),
    },
  });
}
/** Presentation extent of the drawn values; null when they hold no range. */
function derivedDomain(values: readonly number[]): HeatmapColors | null {
  const min = Math.min(...values),
    max = Math.max(...values);
  if (!(min < max)) return null;
  return min < 0 && max > 0
    ? { palette: "diverging", domain: [min, 0, max] }
    : { palette: "sequential", domain: [min, max] };
}
function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}
const numericEpisodeFields = new Set([
  "duration_days",
  "max_drawdown",
  "near_recovery_threshold",
]);
const episodeColumns: ColumnDef<{}, DrawdownEpisode, any>[] = (
  [
    "start",
    "valley",
    "end",
    "duration_days",
    "max_drawdown",
    "near_recovery_threshold",
    "truncated_at_start",
  ] as const
).map((field) => ({
  id: field,
  header: labelFor(field),
  accessorFn: (row: DrawdownEpisode) => row[field],
  cell: ({ getValue }) => {
    const value = getValue() as string | number | boolean | null | undefined;
    return typeof value === "boolean" ? String(value) : formatRaw(value);
  },
  meta: {
    className: numericEpisodeFields.has(field)
      ? "text-right finstack-numeric"
      : "text-left",
  },
}));
/**
 * Presents one native analytics.Performance report: lookback tiles, the
 * summary table envelope, cumulative, drawdown and rolling series, monthly and
 * annual returns and drawdown episodes. Every value is shown as Rust returned
 * it (decimal fractions and ratios, no units inferred); nothing is computed.
 */
export function PerformanceReport({
  report,
  loading,
  error,
  ...props
}: PerformanceReportProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Calculating performance…
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No performance report
      </p>
    );
  return <ReportView report={report} {...props} />;
}
function ReportView({
  report,
  initialTicker = 0,
  monthlyDomain,
  formatValue = String,
  density = "compact",
}: Omit<PerformanceReportProps, "report" | "loading" | "error"> & {
  report: PerformanceReportData;
}) {
  const [selected, setSelected] = useState(initialTicker);
  const ticker = report.tickerNames[selected] ?? "";
  const series = report.tickers[selected];
  const cumulative = useMemo(
    () =>
      datedChart(
        report.tickers.flatMap((entry, index) =>
          datedPoints(
            report.tickerNames[index]!,
            entry.activeDates,
            entry.cumulativeReturns,
          ),
        ),
        "line",
        "Cumulative return (decimal)",
        formatValue,
      ),
    [report, formatValue],
  );
  const perTicker = useMemo(() => {
    if (!series) return null;
    const rolling = (value: PerformanceTickerData["rollingSharpe"]) =>
      datedChart(
        datedPoints(ticker, value.dates, value.values),
        "line",
        labelFor(value.value_column),
        formatValue,
      );
    return {
      drawdown: datedChart(
        datedPoints(ticker, series.activeDates, series.drawdowns),
        "area",
        "Drawdown (decimal)",
        formatValue,
      ),
      sharpe: rolling(series.rollingSharpe),
      volatility: rolling(series.rollingVolatility),
    };
  }, [series, ticker, formatValue]);
  const monthly = useMemo(() => {
    // Columns and rows follow the first appearance of each month and year in native order.
    const data = (report.monthly[selected] ?? []).filter((point) =>
      Number.isFinite(point.value),
    );
    const colors = monthlyDomain ?? derivedDomain(data.map((p) => p.value));
    if (!data.length || !colors)
      return { status: "No range of finite monthly returns to colour" };
    const [low, high] = [colors.domain[0], colors.domain.at(-1)!];
    if (data.some((point) => point.value < low || point.value > high))
      return {
        status: `The supplied colour domain [${colors.domain.join(", ")}] excludes a returned monthly value`,
      };
    return {
      preset: heatmap({
        data,
        x: (p) => p.date.slice(5, 7),
        y: (p) => p.date.slice(0, 4),
        value: (p) => p.value,
        key: (p) => p.date,
        xLabel: "Month",
        yLabel: "Year",
        valueLabel: "Monthly return (decimal)",
        formatValue,
        ...colors,
      }),
    };
  }, [report, selected, monthlyDomain, formatValue]);
  const annual = useMemo(() => {
    const data = (report.annual[selected] ?? []).filter((point) =>
      Number.isFinite(point.value),
    );
    return defineChart({
      marks: [
        barY(data, {
          x: (p) => p.date.slice(0, 4),
          y: (p) => p.value,
          key: (p) => p.date,
          id: "annual-returns",
        }),
      ],
      scales: {
        x: {
          scale: () => scaleBand<string>().padding(0.3),
          axis: { label: "Year (period end)" },
        },
        y: {
          scale: scaleLinear,
          nice: true,
          axis: { label: "Annual return (decimal)" },
        },
      },
      tooltip: {
        use: tooltip,
        content: (marks) => ({
          rows: marks.map((m) => ({
            label: m.datum.date,
            value: formatValue(m.datum.value),
          })),
        }),
      },
    });
  }, [report, selected, formatValue]);
  const fiscal = report.lookback.fiscalYearStart;
  const lookbackIndex = report.lookback.returns.ticker_names.indexOf(ticker);
  return (
    <section
      aria-label="Performance report"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-end gap-4">
        <div className="min-w-48">
          <EnumField
            label="Ticker"
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
          {labelFor(report.frequency)} observations ·{" "}
          {report.activeDates[0] ?? "—"} to {report.activeDates.at(-1) ?? "—"}
        </p>
      </div>
      <Section title={`Lookback returns to ${report.lookback.refDate}`}>
        <div className="grid grid-cols-2 gap-2 @lg:grid-cols-4">
          {lookbackKeys.map((key) => (
            <section
              key={key}
              aria-label={labelFor(key)}
              className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
            >
              <h4 className="text-xs font-medium text-muted-foreground">
                {labelFor(key)}
              </h4>
              <p className="finstack-numeric text-base">
                {display(
                  report.lookback.returns[key][lookbackIndex],
                  formatValue,
                )}
              </p>
            </section>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          Decimal fractions as Rust returned them.{" "}
          {fiscal
            ? `Fiscal year starts month ${fiscal.month}, day ${fiscal.day}.`
            : "Fiscal year is the calendar year (Rust default)."}
        </p>
      </Section>
      <Section title="Summary">
        <TableEnvelope
          envelope={report.summary}
          caption="Performance summary by ticker (native units)"
          captionVisibility="sr-only"
          density={density}
        />
      </Section>
      <Section title="Cumulative returns">
        <FinstackChart
          definition={cumulative}
          ariaLabel="Cumulative returns by ticker"
          height={280}
        />
      </Section>
      {perTicker && series ? (
        <>
          <div className="grid gap-6 @3xl:grid-cols-2">
            <Section title={`${ticker} drawdown`}>
              <FinstackChart
                definition={perTicker.drawdown}
                ariaLabel={`${ticker} drawdown`}
                height={240}
              />
            </Section>
            <Section title={`${ticker} annual returns`}>
              <FinstackChart
                definition={annual}
                ariaLabel={`${ticker} annual returns`}
                height={240}
              />
            </Section>
            <Section
              title={`${ticker} rolling ${labelFor(series.rollingSharpe.value_column)}`}
            >
              <FinstackChart
                definition={perTicker.sharpe}
                ariaLabel={`${ticker} rolling ${series.rollingSharpe.value_column}`}
                height={240}
              />
            </Section>
            <Section
              title={`${ticker} rolling ${labelFor(series.rollingVolatility.value_column)}`}
            >
              <FinstackChart
                definition={perTicker.volatility}
                ariaLabel={`${ticker} rolling ${series.rollingVolatility.value_column}`}
                height={240}
              />
            </Section>
          </div>
          <Section title={`${ticker} monthly returns`}>
            {"preset" in monthly && monthly.preset ? (
              <FinstackChart
                {...monthly.preset}
                ariaLabel={`${ticker} monthly returns heatmap`}
                height={280}
              />
            ) : (
              <p role="status">{monthly.status}</p>
            )}
          </Section>
          <Section title={`${ticker} drawdown episodes`}>
            <FinstackTable
              data={[...series.drawdownEpisodes]}
              columns={episodeColumns}
              getRowId={(row) => row.start}
              caption={`${ticker} drawdown episodes, largest first`}
              captionVisibility="sr-only"
              density={density}
              emptyState="No drawdown episodes returned"
            />
          </Section>
        </>
      ) : (
        <p role="status">No ticker series returned</p>
      )}
    </section>
  );
}
