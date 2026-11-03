"use client";
import { useCallback, useMemo, useState, type ReactNode } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { barX, colorLegend, defineChart, dot, lineY } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type {
  CompanyMetrics,
  RelativeValueReport,
  RelativeValueReportRequest,
} from "@/workers/finstack-contract";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import resultMetadata from "@/lib/finstack/generated/meta/relative_value_report";
import { Badge } from "@/components/ui/badge";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { relativeValueConfigModule } from "./relative-value-config";

type Company = RelativeValueReport["companies"][number];
type MultipleRow = RelativeValueReport["multiples"][number];
type Dimension = RelativeValueReport["dimensions"][number];
type Point = Dimension["points"][number];
type Stats = NonNullable<MultipleRow["peer_stats"]>;
type Extractor = Dimension["y"];
/** Four significant decimals, and never "-0": Rust's signal can be -0.0. */
const defaultNumber = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
/** A supplied 0-1 share as a percent, scaled by Intl for display only. */
const defaultPercentile = new Intl.NumberFormat("en-US", {
  style: "percent",
  maximumFractionDigits: 1,
}).format;
/**
 * No standalone native RelativeValueReportConfig parse exists; Rust
 * validates the config (unknown keys included) when it builds the report and
 * the caller shows its error. Kept stable so the form does not revalidate on
 * every render.
 */
const passThrough = async (json: string) => json;
/** Rust's description of a returned field or enum value, from the generated contract. */
function described(suffix: string, value?: string): string | undefined {
  return resultMetadata.find((entry) =>
    value === undefined
      ? entry.source.endsWith(suffix)
      : entry.source.includes(`/${suffix}/oneOf/`) &&
        "const" in entry &&
        entry.const === value,
  )?.description;
}
const undefinedValue = (reason: string | undefined) => (
  <span className="finstack-na" title={reason}>
    Undefined
  </span>
);
/**
 * A number Rust returned, or Undefined for its null; a live worker may
 * deliver a missing number as NaN. -0 is shown as 0.
 */
function shown(
  value: number | null | undefined,
  format: (value: number) => string,
  reason?: string,
): ReactNode {
  if (value == null || Number.isNaN(value)) return undefinedValue(reason);
  return format(Object.is(value, -0) ? 0 : value);
}
/** The metric an extractor names: a named field, a multiple or a custom key. */
function extractorLabel(extractor: Extractor): string {
  const [kind, name] = Object.entries(extractor)[0] as [string, string];
  return kind === "custom" ? name : labelFor(name);
}
/** The period basis as Rust returned it: ltm, ntm or a custom label. */
function basisLabel(basis: RelativeValueReport["period_basis"]): string {
  return typeof basis === "string" ? labelFor(basis) : basis.custom;
}
const statFields = [
  "count",
  "min",
  "q1",
  "median",
  "q3",
  "max",
  "mean",
  "std_dev",
] as const;
/** One percentile bar: a returned percentile, drawn from zero. */
export interface PercentileBar {
  label: string;
  /** Rust's percentile of the subject among the peers, 0 to 1. */
  percentile: number;
}
/** The multiples Rust placed the subject on, in config order, one bar each. */
export function percentileBars(report: RelativeValueReport): PercentileBar[] {
  return report.multiples.flatMap((row) =>
    row.percentile == null || Number.isNaN(row.percentile)
      ? []
      : [{ label: labelFor(row.multiple), percentile: row.percentile }],
  );
}
/** One scatter point: a returned point, marked as the subject or a peer. */
export interface ScatterPoint extends Point {
  role: string;
}
const subjectRole = "Subject";
const peerRole = "Peer";
/** A dimension's returned points, the subject found by its id. */
export function scatterPoints(
  report: RelativeValueReport,
  dimension: Dimension,
): ScatterPoint[] {
  return dimension.points.map((point) => ({
    ...point,
    role: point.company_id === report.subject_id ? subjectRole : peerRole,
  }));
}
/** The returned fitted values in order of the supplied X: the regression line. */
export function fittedLine(dimension: Dimension): Point[] {
  return dimension.points
    .filter((point) => point.fitted_y != null && !Number.isNaN(point.fitted_y))
    .sort((a, b) => (a.x < b.x ? -1 : a.x > b.x ? 1 : 0));
}
export interface RelativeValueTableProps {
  /** The subject's CompanyMetrics, as the host supplies it. */
  subject: CompanyMetrics;
  /** The universe's CompanyMetrics, in the order the report returns peers. */
  universe: readonly CompanyMetrics[];
  /** Initial RelativeValueReportConfig; remount to load another. Omitted starts empty. */
  defaultConfig?: Record<string, unknown>;
  /** Receives the subject, universe and structurally valid config JSON on Compare; Rust validates them. */
  onSubmit(request: RelativeValueReportRequest): void;
  /** Native statements_analytics.relativeValueReport result; null while none is available. */
  report: RelativeValueReport | null | undefined;
  loading?: boolean;
  /** Native error (a misspelt field, no dimension or multiple, a negative weight), shown as returned. */
  error?: string;
  /** Display text of a returned multiple, metric, statistic, z-score or signal. */
  formatNumber?: (value: number) => string;
  /** Display text of a returned percentile (a 0-1 share); defaults to a percent. */
  formatPercentile?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Compares a host-supplied subject with its peers through Rust's relative
 * value report: the comps table, each multiple's peer statistics and
 * percentile, each scoring dimension's regression scatter and signal, and the
 * composite rich/cheap score. The config is edited with the schema form over
 * the generated contract. Every figure, fitted value and exclusion is Rust's;
 * nothing is computed here.
 */
export function RelativeValueTable({
  subject,
  universe,
  defaultConfig,
  onSubmit,
  report,
  loading,
  error,
  formatNumber = defaultNumber,
  formatPercentile = defaultPercentile,
  density = "compact",
}: RelativeValueTableProps) {
  const [initial] = useState(() => defaultConfig);
  const subjectJson = useMemo(() => JSON.stringify(subject), [subject]);
  const universeJson = useMemo(() => JSON.stringify(universe), [universe]);
  const submit = useCallback(
    (configJson: string) => onSubmit({ subjectJson, universeJson, configJson }),
    [onSubmit, subjectJson, universeJson],
  );
  return (
    <section
      aria-label="Relative value"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !report ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading
            ? "Comparing the subject with its peers…"
            : "No relative value report available."}
        </p>
      ) : (
        <RelativeValueResults
          report={report}
          formatNumber={formatNumber}
          formatPercentile={formatPercentile}
          density={density}
        />
      )}
      <SchemaForm
        module={relativeValueConfigModule}
        defaultValues={initial}
        layout="full"
        explicitNull={false}
        validate={passThrough}
        onSubmit={submit}
        submitLabel="Compare"
      />
    </section>
  );
}
interface ResultProps {
  report: RelativeValueReport;
  formatNumber: (value: number) => string;
  formatPercentile: (value: number) => string;
  density: "compact" | "comfortable";
}
function RelativeValueResults({
  report,
  formatNumber,
  formatPercentile,
  density,
}: ResultProps) {
  const composite = report.composite;
  const compositeReason = report.composite_excluded_reason;
  const headline: readonly (readonly [string, ReactNode])[] = [
    ["subject_id", report.subject_id],
    ["period_basis", basisLabel(report.period_basis)],
    ["peer_count", String(report.peer_count)],
    ["excluded_count", String(report.excluded_count)],
    [
      "composite_score",
      composite
        ? shown(composite.composite_score, formatNumber)
        : undefinedValue(undefined),
    ],
    [
      "confidence",
      composite
        ? shown(composite.confidence, formatNumber)
        : undefinedValue(undefined),
    ],
  ];
  const columns: ColumnDef<{}, Company, any>[] = [
    {
      id: "id",
      header: labelFor("id"),
      accessorFn: (row) => row.id,
      cell: ({ row }) => (
        <span className="inline-flex items-center gap-2">
          {row.original.id}
          {row.original.is_subject && (
            <Badge variant="secondary">{subjectRole}</Badge>
          )}
        </span>
      ),
      meta: { className: "", rowHeader: true },
    },
    ...report.multiples.map((column, index): ColumnDef<{}, Company, any> => ({
      id: column.multiple,
      header: labelFor(column.multiple),
      accessorFn: (row) => row.values[index],
      cell: ({ getValue }) =>
        shown(
          getValue() as number | null,
          formatNumber,
          described("RelativeValueCompany/properties/values"),
        ),
      meta: { className: "text-right finstack-numeric" },
    })),
  ];
  return (
    <div className="min-w-0 space-y-6">
      <dl
        aria-label={`Relative value of ${report.subject_id}`}
        className="grid grid-cols-1 gap-2 @md:grid-cols-3 @3xl:grid-cols-6"
      >
        {headline.map(([field, value]) => (
          <div
            key={field}
            className="rounded-md border border-border bg-card px-3 py-2"
          >
            <dt className="text-xs text-muted-foreground">{labelFor(field)}</dt>
            <dd className="finstack-numeric text-base">{value}</dd>
          </div>
        ))}
      </dl>
      {compositeReason && (
        <p className="text-sm text-muted-foreground">
          No composite score: {labelFor(compositeReason)}.{" "}
          {described("CompositeExclusion", compositeReason)}
        </p>
      )}
      {report.multiples.length > 0 && (
        <FinstackTable
          data={report.companies}
          columns={columns}
          getRowId={(row) => row.id}
          caption={`Comparable companies, ${basisLabel(report.period_basis)}`}
          density={density}
        />
      )}
      {report.multiples.length > 0 && (
        <Multiples
          report={report}
          formatNumber={formatNumber}
          formatPercentile={formatPercentile}
        />
      )}
      {report.dimensions.map((dimension, index) => (
        <DimensionPanel
          key={index}
          report={report}
          dimension={dimension}
          formatNumber={formatNumber}
          formatPercentile={formatPercentile}
        />
      ))}
    </div>
  );
}
function StatsList({
  stats,
  label,
  formatNumber,
}: {
  stats: Stats | null | undefined;
  label: string;
  formatNumber: (value: number) => string;
}) {
  if (!stats)
    return (
      <p className="text-xs text-muted-foreground">
        Peer statistics:{" "}
        {undefinedValue(
          described("RelativeValueMultiple/properties/peer_stats"),
        )}
      </p>
    );
  return (
    <dl
      aria-label={label}
      className="grid grid-cols-2 gap-x-3 gap-y-1 text-xs @md:grid-cols-4"
    >
      {statFields.map((field) => (
        <div key={field} className="flex justify-between gap-2">
          <dt className="text-muted-foreground">{labelFor(field)}</dt>
          <dd className="finstack-numeric">
            {field === "count"
              ? String(stats.count)
              : shown(stats[field], formatNumber)}
          </dd>
        </div>
      ))}
    </dl>
  );
}
function Multiples({
  report,
  formatNumber,
  formatPercentile,
}: Omit<ResultProps, "density">) {
  const bars = useMemo(() => percentileBars(report), [report]);
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          barX(bars, {
            y: (bar: PercentileBar) => bar.label,
            x1: () => 0,
            x2: (bar: PercentileBar) => bar.percentile,
            key: (bar: PercentileBar) => bar.label,
            id: "percentile",
          }),
        ],
        scales: {
          x: {
            // A percentile is a share of the peers: the axis always spans 0 to 1.
            scale: scaleLinear().domain([0, 1]),
            axis: { label: "Peers at or below the subject" },
          },
          y: {
            scale: () =>
              scaleBand<string>()
                .domain(bars.map((bar) => bar.label))
                .padding(0.3),
          },
        },
        tooltip: {
          use: tooltip,
          content: (marks) => ({
            rows: marks.map((m) => ({
              label: m.datum.label,
              value: formatPercentile(m.datum.percentile),
            })),
          }),
        },
      }),
    [bars, formatPercentile],
  );
  return (
    <section aria-label="Multiples against peers" className="min-w-0 space-y-3">
      <h3 className="text-sm font-medium">Multiples against peers</h3>
      <div className="grid grid-cols-1 gap-3 @3xl:grid-cols-2">
        {report.multiples.map((row) => {
          const name = labelFor(row.multiple);
          return (
            <article
              key={row.multiple}
              aria-label={name}
              className="min-w-0 space-y-2 rounded-md border border-border bg-card px-3 py-2"
            >
              <h4 className="text-sm font-medium">{name}</h4>
              <dl className="grid grid-cols-3 gap-2 text-xs">
                {(
                  [
                    [
                      "subject_value",
                      shown(
                        row.subject_value,
                        formatNumber,
                        described(
                          "RelativeValueMultiple/properties/subject_value",
                        ),
                      ),
                    ],
                    [
                      "percentile",
                      shown(
                        row.percentile,
                        formatPercentile,
                        described(
                          "RelativeValueMultiple/properties/percentile",
                        ),
                      ),
                    ],
                    [
                      "z_score",
                      shown(
                        row.z_score,
                        formatNumber,
                        described("RelativeValueMultiple/properties/z_score"),
                      ),
                    ],
                  ] as const
                ).map(([field, value]) => (
                  <div key={field}>
                    <dt className="text-muted-foreground">{labelFor(field)}</dt>
                    <dd className="finstack-numeric text-sm">{value}</dd>
                  </div>
                ))}
              </dl>
              <StatsList
                stats={row.peer_stats}
                label={`${name} peer statistics`}
                formatNumber={formatNumber}
              />
            </article>
          );
        })}
      </div>
      {bars.length > 0 && (
        <figure className="min-w-0">
          <figcaption className="text-xs text-muted-foreground">
            Percentile of the subject among its peers, by multiple.
          </figcaption>
          <FinstackChart
            definition={definition}
            ariaLabel={`Percentile of ${report.subject_id} by multiple`}
            height={240}
          />
        </figure>
      )}
    </section>
  );
}
function DimensionPanel({
  report,
  dimension,
  formatNumber,
  formatPercentile,
}: {
  report: RelativeValueReport;
  dimension: Dimension;
  formatNumber: (value: number) => string;
  formatPercentile: (value: number) => string;
}) {
  const field = (name: string) =>
    described(`RelativeValueDimension/properties/${name}`);
  const regression = dimension.regression;
  const reason = dimension.excluded_reason;
  const xLabel = dimension.x ? extractorLabel(dimension.x) : undefined;
  const yLabel = extractorLabel(dimension.y);
  const figures: readonly (readonly [string, ReactNode])[] = [
    ["weight", shown(dimension.weight, formatNumber)],
    ["direction", labelFor(dimension.direction)],
    ["subject_y", shown(dimension.subject_y, formatNumber, field("subject_y"))],
    ...(dimension.x
      ? ([
          [
            "subject_x",
            shown(dimension.subject_x, formatNumber, field("subject_x")),
          ],
        ] as const)
      : []),
    [
      "percentile",
      shown(dimension.percentile, formatPercentile, field("percentile")),
    ],
    ["z_score", shown(dimension.z_score, formatNumber, field("z_score"))],
    ["signal", shown(dimension.signal, formatNumber, field("signal"))],
    ...(regression
      ? ([
          ["r_squared", shown(regression.r_squared, formatNumber)],
          ["fitted_value", shown(regression.fitted_value, formatNumber)],
          ["residual", shown(regression.residual, formatNumber)],
          ["n", String(regression.n)],
        ] as const)
      : []),
  ];
  const points = useMemo(
    () => scatterPoints(report, dimension),
    [report, dimension],
  );
  const line = useMemo(() => fittedLine(dimension), [dimension]);
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          lineY(line, {
            x: (point: Point) => point.x,
            y: (point: Point) => point.fitted_y as number,
            key: (point: Point) => point.company_id,
            id: "fitted",
            stroke: "var(--muted-foreground)",
          }),
          dot(points, {
            x: (point: ScatterPoint) => point.x,
            y: (point: ScatterPoint) => point.y,
            color: (point: ScatterPoint) => point.role,
            key: (point: ScatterPoint) => point.company_id,
            id: "companies",
            r: 4,
          }),
        ],
        scales: {
          x: { scale: scaleLinear, nice: true, axis: { label: xLabel ?? "" } },
          y: { scale: scaleLinear, nice: true, axis: { label: yLabel } },
        },
        color: { domain: [subjectRole, peerRole], legend: colorLegend() },
        tooltip: {
          use: tooltip,
          content: (marks) => ({
            rows: marks.flatMap((m) => {
              const point = m.datum as ScatterPoint;
              return "role" in point
                ? [
                    {
                      label: point.company_id,
                      value: `${xLabel} ${formatNumber(point.x)}, ${yLabel} ${formatNumber(point.y)}`,
                    },
                  ]
                : [];
            }),
          }),
        },
      }),
    [points, line, xLabel, yLabel, formatNumber],
  );
  return (
    <section
      aria-label={dimension.label}
      className="min-w-0 space-y-3 rounded-md border border-border bg-card px-3 py-2"
    >
      <h3 className="text-sm font-medium">
        {dimension.label}
        <span className="ml-2 text-xs font-normal text-muted-foreground">
          {xLabel ? `${yLabel} on ${xLabel}` : `${yLabel} against peers`}
        </span>
      </h3>
      {reason && (
        <p className="text-xs text-muted-foreground">
          Excluded from the composite: {labelFor(reason)}.{" "}
          {described("DimensionExclusion", reason)}
        </p>
      )}
      <dl className="grid grid-cols-2 gap-2 text-xs @md:grid-cols-4 @3xl:grid-cols-6">
        {figures.map(([name, value]) => (
          <div key={name}>
            <dt className="text-muted-foreground">{labelFor(name)}</dt>
            <dd className="finstack-numeric text-sm">{value}</dd>
          </div>
        ))}
      </dl>
      <StatsList
        stats={dimension.peer_stats}
        label={`${dimension.label} peer statistics`}
        formatNumber={formatNumber}
      />
      {points.length > 0 && (
        <figure className="min-w-0">
          <figcaption className="text-xs text-muted-foreground">
            {line.length > 0
              ? "Peers and the subject, with Rust's fitted line."
              : "Peers and the subject; Rust returned no regression."}
          </figcaption>
          <FinstackChart
            definition={definition}
            ariaLabel={`${dimension.label}: ${yLabel} on ${xLabel}`}
            height={320}
          />
        </figure>
      )}
    </section>
  );
}
