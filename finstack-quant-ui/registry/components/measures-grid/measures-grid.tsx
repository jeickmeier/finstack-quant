"use client";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import { MeasureValue } from "../../primitives/measure-value/measure-value";
import {
  ValuationSummary,
  type ValuationSummaryProps,
} from "../valuation-summary/valuation-summary";
import type { ColumnDef } from "@tanstack/react-table";
import type { MetricMetadata } from "finstack-quant-wasm";
export interface MeasuresGridProps extends ValuationSummaryProps {
  /** Rust-supplied per-key metadata for the primary result; absent records keep keys raw. */
  metadata?: readonly MetricMetadata[];
  /** Rust-supplied per-key metadata for the comparison result, independent of the primary sidecar. */
  comparisonMetadata?: readonly MetricMetadata[];
  /** Optional source-backed units by complete metric key. Values are always displayed raw. */
  units?: Readonly<Record<string, { label: string; source: string }>>;
  comparisonUnits?: Readonly<Record<string, { label: string; source: string }>>;
}
type MeasureRow = {
  key: string;
  value: number | undefined;
  comparison: number | undefined;
};
function measureUnit(
  key: string,
  metadata: readonly MetricMetadata[] | undefined,
  units: MeasuresGridProps["units"],
) {
  const supplied = units?.[key];
  if (supplied) return supplied;
  const unit = metadata?.find((entry) => entry.key === key)?.unit;
  return unit && unit !== "unknown"
    ? {
        label: unit,
        source: "finstack_quant_valuations::pricer::metric_metadata",
      }
    : undefined;
}
/** Normalize display widths only; native descriptors decide bucket eligibility and series identity. */
function bucketBars(
  measures: Record<string, number> | undefined,
  units: MeasuresGridProps["units"],
  metadata: readonly MetricMetadata[] | undefined,
) {
  const series = new Map<string, [string, number][]>();
  for (const [key, value] of Object.entries(measures ?? {})) {
    const descriptor = metadata?.find((entry) => entry.key === key);
    if (!descriptor?.bucketed || !Number.isFinite(value)) continue;
    const unit = measureUnit(key, metadata, units);
    const id = JSON.stringify([
      descriptor.metric,
      descriptor.components[0],
      unit?.label,
      unit?.source,
    ]);
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
      className="relative ml-2 inline-block h-2 w-16 shrink-0 align-middle"
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
/** Group by the Rust-supplied native group while retaining the complete returned key. */
export function groupMeasures(
  props: Pick<
    MeasuresGridProps,
    "result" | "compareTo" | "metadata" | "comparisonMetadata"
  >,
) {
  const grouped = new Map<string, MeasureRow[]>();
  const keys = new Set([
    ...Object.keys(props.result?.measures ?? {}),
    ...Object.keys(props.compareTo?.measures ?? {}),
  ]);
  for (const key of keys) {
    const group =
      props.metadata?.find((entry) => entry.key === key)?.group ??
      props.comparisonMetadata?.find((entry) => entry.key === key)?.group ??
      "Group unavailable";
    const rows = grouped.get(group) ?? [];
    rows.push({
      key,
      value: props.result?.measures[key],
      comparison: props.compareTo?.measures[key],
    });
    grouped.set(group, rows);
  }
  return [...grouped].map(([group, rows]) => ({ group, rows }));
}
/** Supplied measures and independent comparison context, using the unlinked core table. */
export function MeasuresGrid(props: MeasuresGridProps) {
  const grouped = groupMeasures(props);
  const bars = bucketBars(props.result?.measures, props.units, props.metadata);
  const comparisonBars = bucketBars(
    props.compareTo?.measures,
    props.comparisonUnits,
    props.comparisonMetadata,
  );
  const noUnits =
    !Object.keys(props.units ?? {}).length &&
    !Object.keys(props.comparisonUnits ?? {}).length &&
    !Object.keys(props.result?.measures ?? {}).some((key) =>
      measureUnit(key, props.metadata, props.units),
    ) &&
    !Object.keys(props.compareTo?.measures ?? {}).some((key) =>
      measureUnit(key, props.comparisonMetadata, props.comparisonUnits),
    );
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
      header: props.compareTo === undefined ? "Value" : "Valuation",
      accessorFn: (row) => row.value,
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => (
        <>
          <MeasureValue
            value={context.row.original.value}
            unit={measureUnit(
              context.row.original.key,
              props.metadata,
              props.units,
            )}
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
            header: "Comparison",
            accessorFn: (row: MeasureRow) => row.comparison,
            meta: { className: "text-right finstack-numeric" },
            cell: (context: { row: { original: MeasureRow } }) => (
              <>
                <MeasureValue
                  value={context.row.original.comparison}
                  unit={measureUnit(
                    context.row.original.key,
                    props.comparisonMetadata,
                    props.comparisonUnits,
                  )}
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
        formattedValue={props.formattedValue}
        comparisonFormattedValue={props.comparisonFormattedValue}
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
