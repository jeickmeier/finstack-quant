"use client";
import { useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barY, defineChart } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type {
  AggregatedMetric,
  MetricMetadata,
  PortfolioMetrics as PortfolioMetricsResult,
  PortfolioMetricSeriesEntry,
} from "finstack-quant-wasm";
import { formatRaw } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** The parts of a returned portfolio valuation the metrics view presents. */
export interface PortfolioMetricsData {
  /** Position ids in spec order (Portfolio#positionIds). */
  positionIds: readonly string[];
  /** Entity ids in registration order (Portfolio#entityIds); entity columns follow it. */
  entityIds: readonly string[];
  /** portfolio.aggregateMetrics as returned. */
  metrics: PortfolioMetricsResult;
}
/** One base metric's decoded series (portfolio.portfolioMetricsSeries), in the order Rust returned it. */
export interface PortfolioMetricSeriesData {
  base: string;
  entries: readonly PortfolioMetricSeriesEntry[];
}
export interface PortfolioMetricsProps {
  /** The returned metrics; null while none are available. */
  report: PortfolioMetricsData | null;
  /** A bucketed series to chart; omitted or null shows no chart. */
  series?: PortfolioMetricSeriesData | null;
  /** Rust metricMetadata for the metric keys; labels the series base when present. */
  metadata?: readonly MetricMetadata[];
  /** Position shown first in the by-position table; omitted is the first position. */
  initialPosition?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  loading?: boolean;
  /** The native error message, shown instead of metrics. */
  error?: string;
  density?: "compact" | "comfortable";
}
function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="min-w-0 space-y-2">
      <h3 className="text-sm font-medium">{title}</h3>
      {children}
    </section>
  );
}
/** Host display text for finite numbers; anything else is shown as returned. */
function display(value: unknown, format: (value: number) => string) {
  return typeof value === "number" && Number.isFinite(value)
    ? format(value)
    : formatRaw(value as string | number | null | undefined);
}
type SeriesPoint = { index: number; bucket: string; total: number };
/** Bars of one series group in the returned entry order; no tenor sort is applied. */
function seriesChart(
  points: readonly SeriesPoint[],
  valueLabel: string,
  format: (value: number) => string,
) {
  return defineChart({
    marks: [
      barY(points, {
        x: (p) => p.bucket,
        y: (p) => p.total,
        key: (p) => String(p.index),
        id: "metric-series",
      }),
    ],
    scales: {
      x: {
        scale: () =>
          scaleBand<string>()
            .domain(points.map((p) => p.bucket))
            .padding(0.3),
        axis: { label: "Bucket (Rust order)" },
      },
      y: { scale: scaleLinear, nice: true, axis: { label: valueLabel } },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.map((m) => ({
          label: m.datum.bucket,
          value: format(m.datum.total),
        })),
      }),
    },
  });
}
/**
 * Presents native aggregated portfolio metrics: a bucketed series chart in
 * the order Rust returned it, the aggregated totals with their by-entity
 * values, each position's raw metrics, and the unaggregated, skipped and
 * degraded lists. Nothing is summed, converted or sorted.
 */
export function PortfolioMetrics({
  report,
  loading,
  error,
  ...props
}: PortfolioMetricsProps) {
  if (error)
    return (
      <p role="alert" className="font-sans text-sm text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        Aggregating metrics…
      </p>
    );
  if (!report)
    return (
      <p role="status" className="font-sans text-sm text-muted-foreground">
        No portfolio metrics
      </p>
    );
  return <MetricsView report={report} {...props} />;
}
function MetricsView({
  report,
  series,
  metadata,
  initialPosition,
  formatValue = String,
  density = "compact",
}: Omit<PortfolioMetricsProps, "report" | "loading" | "error"> & {
  report: PortfolioMetricsData;
}) {
  const { metrics, entityIds, positionIds } = report;
  const [selected, setSelected] = useState(
    initialPosition ?? positionIds[0] ?? "",
  );
  const baseLabel = series
    ? labelFor(
        metadata?.find((entry) => entry.key === series.base)?.metric ??
          series.base,
      )
    : "";
  // Group by the first key component (the curve); entries keep Rust order within and across groups.
  const groups = useMemo(() => {
    if (!series) return [];
    const byGroup = Map.groupBy(
      series.entries.flatMap((entry, index) =>
        Number.isFinite(entry.total)
          ? [
              {
                group: entry.components.length > 1 ? entry.components[0]! : "",
                point: {
                  index,
                  bucket: (entry.components.length > 1
                    ? entry.components.slice(1)
                    : entry.components
                  ).join(" · "),
                  total: entry.total,
                },
              },
            ]
          : [],
      ),
      (item) => item.group,
    );
    return [...byGroup].map(([group, items]) => ({
      group,
      definition: seriesChart(
        items.map((item) => item.point),
        `${baseLabel} total`,
        formatValue,
      ),
    }));
  }, [series, baseLabel, formatValue]);
  const aggregatedColumns = useMemo<ColumnDef<{}, AggregatedMetric, any>[]>(
    () => [
      {
        id: "metric",
        header: "Metric",
        accessorFn: (row) => row.metric_id,
        meta: { className: "text-left font-mono" },
      },
      {
        id: "total",
        header: "Total",
        accessorFn: (row) => row.total,
        cell: ({ getValue }) => display(getValue(), formatValue),
        meta: { className: "text-right finstack-numeric" },
      },
      ...entityIds.map((entity): ColumnDef<{}, AggregatedMetric, any> => ({
        id: `entity:${entity}`,
        header: entity,
        accessorFn: (row) => row.by_entity[entity],
        cell: ({ getValue }) => display(getValue(), formatValue),
        meta: { className: "text-right finstack-numeric" },
      })),
    ],
    [entityIds, formatValue],
  );
  const positionColumns = useMemo<
    ColumnDef<{}, { key: string; value: number }, any>[]
  >(
    () => [
      {
        id: "metric",
        header: "Metric",
        accessorFn: (row) => row.key,
        meta: { className: "text-left font-mono" },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
        cell: ({ getValue }) => display(getValue(), formatValue),
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [formatValue],
  );
  const position = metrics.by_position[selected];
  const unaggregated = metrics.unaggregated_metrics ?? [];
  const skipped = metrics.skipped_metrics ?? [];
  const degraded = metrics.degraded_positions ?? [];
  return (
    <section
      aria-label="Portfolio metrics"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      {series && (
        <Section title={`${baseLabel} by bucket`}>
          {groups.length ? (
            <div className="grid gap-4 @3xl:grid-cols-2">
              {groups.map(({ group, definition }) => (
                <figure key={group} className="min-w-0 space-y-1">
                  {group && (
                    <figcaption className="font-mono text-xs text-muted-foreground">
                      {group}
                    </figcaption>
                  )}
                  <FinstackChart
                    definition={definition}
                    ariaLabel={`${series.base}${group ? ` ${group}` : ""} portfolio totals by bucket`}
                    height={220}
                  />
                </figure>
              ))}
            </div>
          ) : (
            <p role="status">No finite series entries returned</p>
          )}
          <p className="text-xs text-muted-foreground">
            Buckets appear in the order Rust returned them; no tenor sort is
            applied.
          </p>
        </Section>
      )}
      <Section title="Aggregation notes">
        <dl className="grid gap-2 @lg:grid-cols-3">
          <div className="space-y-1">
            <dt className="text-xs text-muted-foreground">
              Unaggregated metrics
            </dt>
            <dd className="font-mono">
              {unaggregated.length ? unaggregated.join(", ") : "None"}
            </dd>
          </div>
          <div className="space-y-1">
            <dt className="text-xs text-muted-foreground">Skipped values</dt>
            <dd>
              {skipped.length ? (
                <ul>
                  {skipped.map((entry) => (
                    <li
                      key={`${entry.position_id}|${entry.metric_id}`}
                      className="font-mono"
                    >
                      {entry.position_id} · {entry.metric_id} ·{" "}
                      {formatRaw(entry.value)}
                    </li>
                  ))}
                </ul>
              ) : (
                "None"
              )}
            </dd>
          </div>
          <div className="space-y-1">
            <dt className="text-xs text-muted-foreground">
              Degraded positions
            </dt>
            <dd className="font-mono">
              {degraded.length ? degraded.join(", ") : "None"}
            </dd>
          </div>
        </dl>
      </Section>
      <Section title="Aggregated metrics">
        <FinstackTable
          data={Object.values(metrics.aggregated)}
          columns={aggregatedColumns}
          getRowId={(row) => row.metric_id}
          caption="Aggregated portfolio metrics in the base currency, with by-entity values"
          captionVisibility="sr-only"
          density={density}
          emptyState="No aggregated metrics returned"
        />
      </Section>
      <Section title="Metrics by position">
        <div className="max-w-xs">
          <EnumField
            label="Position"
            value={selected}
            onValueChange={setSelected}
            options={positionIds.map((id) => ({ value: id, label: id }))}
            layout="select"
          />
        </div>
        {position ? (
          <>
            <p className="text-xs text-muted-foreground">
              Raw position metrics; native currency {position.currency}.
            </p>
            <FinstackTable
              data={Object.entries(position.metrics).map(([key, value]) => ({
                key,
                value,
              }))}
              columns={positionColumns}
              getRowId={(row) => row.key}
              caption={`Metrics of position ${selected}`}
              captionVisibility="sr-only"
              density={density}
              emptyState="No metrics returned for this position"
            />
          </>
        ) : (
          <p role="status">No metrics returned for this position</p>
        )}
      </Section>
    </section>
  );
}
