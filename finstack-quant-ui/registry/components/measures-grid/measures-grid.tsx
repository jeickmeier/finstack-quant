"use client";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import { MeasureValue } from "../../primitives/measure-value/measure-value";
import {
  ValuationSummary,
  type ValuationSummaryProps,
} from "../valuation-summary/valuation-summary";
import type { ColumnDef } from "@tanstack/react-table";
export interface MeasuresGridProps extends ValuationSummaryProps {
  /** Pass the groups returned by listStandardMetricsGrouped; no local financial taxonomy. */
  groups: Readonly<Record<string, readonly string[]>>;
  /** Optional source-backed units by complete metric key. Values are always displayed raw. */
  units?: Readonly<Record<string, { label: string; source: string }>>;
  comparisonUnits?: Readonly<Record<string, { label: string; source: string }>>;
}
type MeasureRow = {
  key: string;
  value: number | undefined;
  comparison: number | undefined;
};
/** Normalize display widths only; qualified native bucket labels remain opaque. */
function bucketBars(
  measures: Record<string, number> | undefined,
  units: MeasuresGridProps["units"],
) {
  const series = new Map<string, [string, number][]>();
  for (const [key, value] of Object.entries(measures ?? {})) {
    const parts = key.split("::");
    if (
      parts.length !== 3 ||
      !["bucketed_dv01", "bucketed_cs01"].includes(parts[0]) ||
      !parts[1] ||
      !parts[2] ||
      !Number.isFinite(value)
    )
      continue;
    const unit = units?.[key];
    const id = JSON.stringify([parts[0], parts[1], unit?.label, unit?.source]);
    const entries = series.get(id) ?? [];
    entries.push([key, value]);
    series.set(id, entries);
  }
  const bars = new Map<string, number>();
  for (const entries of series.values()) {
    const maximum = entries.reduce(
      (max, [, value]) => Math.max(max, Math.abs(value)),
      0,
    );
    for (const [key, value] of entries)
      bars.set(key, maximum === 0 ? 0 : value / maximum);
  }
  return bars;
}
function BucketBar({ ratio }: { ratio: number | undefined }) {
  if (ratio === undefined) return null;
  const width = Math.abs(ratio) * 50;
  return (
    <div
      aria-hidden="true"
      className="relative ml-auto mt-1 h-2 w-24 max-w-full"
      data-bucket-bar
    >
      <span className="absolute inset-y-0 left-1/2 border-l border-muted-foreground" />
      {ratio !== 0 && (
        <span
          data-bucket-bar-fill
          className="absolute top-0.5 h-1 bg-chart-1"
          style={{
            left: `${ratio < 0 ? 50 - width : 50}%`,
            width: `${width}%`,
          }}
        />
      )}
    </div>
  );
}
/** Group by the canonical metric family prefix while retaining the complete returned key. */
export function groupMeasures(
  props: Pick<MeasuresGridProps, "result" | "compareTo" | "groups">,
) {
  const grouped = new Map<string, MeasureRow[]>();
  const keys = new Set([
    ...Object.keys(props.result?.measures ?? {}),
    ...Object.keys(props.compareTo?.measures ?? {}),
  ]);
  for (const key of keys) {
    const matches = Object.entries(props.groups).filter(([, metrics]) =>
      metrics.some((metric) => key === metric || key.startsWith(`${metric}::`)),
    );
    const group = matches.length === 1 ? matches[0][0] : "Group unavailable";
    const rows = grouped.get(group) ?? [];
    rows.push({
      key,
      value: props.result?.measures[key],
      comparison: props.compareTo?.measures[key],
    });
    grouped.set(group, rows);
  }
  return [...new Set([...Object.keys(props.groups), "Group unavailable"])]
    .filter((group) => grouped.has(group))
    .map((group) => ({ group, rows: grouped.get(group)! }));
}
/** Supplied measures and independent comparison context, using the unlinked core table. */
export function MeasuresGrid(props: MeasuresGridProps) {
  const grouped = groupMeasures(props);
  const bars = bucketBars(props.result?.measures, props.units);
  const comparisonBars = bucketBars(
    props.compareTo?.measures,
    props.comparisonUnits,
  );
  const noUnits =
    !Object.keys(props.units ?? {}).length &&
    !Object.keys(props.comparisonUnits ?? {}).length;
  const columns = (group: string): ColumnDef<{}, MeasureRow, any>[] => [
    {
      id: "key",
      accessorFn: (row) => row.key,
      header: () => <span className="font-sans">{group}</span>,
      meta: { className: "text-left" },
      cell: (context) => (
        <span className="finstack-measure-key font-mono">
          {context.getValue()}
        </span>
      ),
    },
    {
      id: "value",
      header:
        props.compareTo === undefined ? "Raw value" : "Valuation · raw value",
      accessorFn: (row) => row.value,
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => (
        <>
          <MeasureValue
            value={context.row.original.value}
            unit={props.units?.[context.row.original.key]}
            showUnavailableUnit={!noUnits}
          />
          <BucketBar ratio={bars.get(context.row.original.key)} />
        </>
      ),
    },
    ...(props.compareTo === undefined
      ? []
      : [
          {
            id: "comparison",
            header: "Comparison · raw value",
            accessorFn: (row: MeasureRow) => row.comparison,
            meta: { className: "text-right finstack-numeric" },
            cell: (context: { row: { original: MeasureRow } }) => (
              <>
                <MeasureValue
                  value={context.row.original.comparison}
                  unit={props.comparisonUnits?.[context.row.original.key]}
                  showUnavailableUnit={!noUnits}
                />
                <BucketBar
                  ratio={comparisonBars.get(context.row.original.key)}
                />
              </>
            ),
          },
        ]),
  ];
  return (
    <div
      className="finstack-measures font-sans text-foreground"
      data-density={props.density}
    >
      <ValuationSummary
        result={props.result}
        compareTo={props.compareTo}
        compact
        model={props.model}
        comparisonModel={props.comparisonModel}
        density={props.density}
        loading={props.loading}
        error={props.error}
      />
      {grouped.length ? (
        grouped.map(({ group, rows }) => (
          <section
            key={group}
            className="finstack-measures__group min-w-0"
            aria-label={`${group} measures`}
          >
            <FinstackTable
              data={rows}
              columns={columns(group)}
              getRowId={(row) => row.key}
              caption={`${group} measures`}
              captionVisibility="sr-only"
            />
          </section>
        ))
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          No measures supplied
        </p>
      )}
      {grouped.length > 0 && noUnits && (
        <p className="text-xs text-muted-foreground">
          Raw measure values · units unavailable
        </p>
      )}
      {(bars.size > 0 || comparisonBars.size > 0) && (
        <p className="text-xs text-muted-foreground">
          Bucket bars center on zero. Scales are separate for each risk family,
          identifier, supplied unit and valuation column.
        </p>
      )}
    </div>
  );
}
