"use client";
import { useState } from "react";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
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
  /** Optional source-backed units by complete metric key. Exact values remain available. */
  units?: Readonly<Record<string, { label: string; source: string }>>;
  comparisonUnits?: Readonly<Record<string, { label: string; source: string }>>;
}
type MeasureRow = {
  key: string;
  value: number | undefined;
  comparison: number | undefined;
};
type MeasureSection = {
  id: string;
  title: string;
  rows: MeasureRow[];
  bucketed: boolean;
};
const metricLabels: Readonly<Record<string, string>> = {
  ytm: "Yield to maturity",
  duration_mod: "Modified duration",
  dv01: "DV01",
  bucketed_dv01: "Bucketed DV01",
  bucketed_cs01: "Bucketed CS01",
};
function descriptorFor(key: string, props: MeasuresGridProps) {
  return (
    props.metadata?.find((entry) => entry.key === key) ??
    props.comparisonMetadata?.find((entry) => entry.key === key)
  );
}
function measureLabel(key: string, props: MeasuresGridProps) {
  const descriptor = descriptorFor(key, props);
  if (!descriptor) return key;
  if (descriptor.bucketed && descriptor.components.length === 2)
    return descriptor.components[1];
  const metric = metricLabels[descriptor.metric];
  if (!metric) return key;
  if (descriptor.components.length === 0) return metric;
  if (descriptor.components.length === 1)
    return `${metric} · ${descriptor.components[0]}`;
  return key;
}
function tenorYears(label: string) {
  const match = /^(\d+(?:\.\d+)?)([dwmy])$/.exec(label);
  if (!match) return undefined;
  const factors = { d: 1 / 365, w: 7 / 365, m: 1 / 12, y: 1 };
  return Number(match[1]) * factors[match[2] as keyof typeof factors];
}
function sortedBucketRows(rows: MeasureRow[], props: MeasuresGridProps) {
  return [...rows].sort((left, right) => {
    const leftLabel = measureLabel(left.key, props);
    const rightLabel = measureLabel(right.key, props);
    const leftTenor = tenorYears(leftLabel);
    const rightTenor = tenorYears(rightLabel);
    if (leftTenor !== undefined && rightTenor !== undefined)
      return leftTenor - rightTenor;
    if (leftTenor !== undefined) return -1;
    if (rightTenor !== undefined) return 1;
    return leftLabel.localeCompare(rightLabel, undefined, { numeric: true });
  });
}
function displaySections(rows: MeasureRow[], props: MeasuresGridProps) {
  const sections = new Map<string, MeasureSection>();
  const scalars: MeasureRow[] = [];
  for (const row of rows) {
    const descriptor = descriptorFor(row.key, props);
    if (!descriptor?.bucketed || descriptor.components.length !== 2) {
      scalars.push(row);
      continue;
    }
    const id = JSON.stringify([descriptor.metric, descriptor.components[0]]);
    const section = sections.get(id) ?? {
      id,
      title: `${metricLabels[descriptor.metric] ?? descriptor.metric} · ${descriptor.components[0]}`,
      rows: [],
      bucketed: true,
    };
    section.rows.push(row);
    sections.set(id, section);
  }
  return [
    ...(scalars.length
      ? [
          {
            id: "other",
            title: "Other measures",
            rows: scalars,
            bucketed: false,
          },
        ]
      : []),
    ...[...sections.values()].map((section) => ({
      ...section,
      rows: sortedBucketRows(section.rows, props),
    })),
  ];
}
function shortenedValue(value: number | undefined) {
  if (value === undefined || !Number.isFinite(value)) return undefined;
  return Number(value.toPrecision(6)).toString();
}
function isZeroOnly(row: MeasureRow, comparing: boolean) {
  return comparing ? row.value === 0 && row.comparison === 0 : row.value === 0;
}
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
  return [...grouped]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([group, rows]) => ({ group, rows }));
}
/** Supplied measures and independent comparison context, using the unlinked core table. */
export function MeasuresGrid(props: MeasuresGridProps) {
  const grouped = groupMeasures(props);
  const [showKeys, setShowKeys] = useState(false);
  const [showExact, setShowExact] = useState(false);
  const [hideZeroRows, setHideZeroRows] = useState(false);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const noUnits =
    !Object.keys(props.units ?? {}).length &&
    !Object.keys(props.comparisonUnits ?? {}).length &&
    !Object.keys(props.result?.measures ?? {}).some((key) =>
      measureUnit(key, props.metadata, props.units),
    ) &&
    !Object.keys(props.compareTo?.measures ?? {}).some((key) =>
      measureUnit(key, props.comparisonMetadata, props.comparisonUnits),
    );
  const columns = (bucketed: boolean): ColumnDef<{}, MeasureRow, any>[] => [
    {
      id: "key",
      accessorFn: (row) => row.key,
      header: bucketed ? "Tenor" : "Measure",
      meta: { className: "text-left" },
      cell: (context) => {
        const key = context.row.original.key;
        const label = showKeys ? key : measureLabel(key, props);
        return (
          <span
            className={`finstack-measure-key ${showKeys ? "font-mono text-xs" : "font-sans"}`}
            title={key}
          >
            {label}
          </span>
        );
      },
    },
    {
      id: "value",
      header: props.compareTo === undefined ? "Value" : "Valuation",
      accessorFn: (row) => row.value,
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => (
        <MeasureValue
          value={context.row.original.value}
          unit={
            context.row.original.value === undefined
              ? undefined
              : measureUnit(
                  context.row.original.key,
                  props.metadata,
                  props.units,
                )
          }
          displayText={
            showExact ? undefined : shortenedValue(context.row.original.value)
          }
          showUnavailableUnit={
            context.row.original.value !== undefined && !noUnits
          }
        />
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
              <MeasureValue
                value={context.row.original.comparison}
                unit={
                  context.row.original.comparison === undefined
                    ? undefined
                    : measureUnit(
                        context.row.original.key,
                        props.comparisonMetadata,
                        props.comparisonUnits,
                      )
                }
                displayText={
                  showExact
                    ? undefined
                    : shortenedValue(context.row.original.comparison)
                }
                showUnavailableUnit={
                  context.row.original.comparison !== undefined && !noUnits
                }
              />
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
      {grouped.length > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-y border-border py-2 print:hidden">
          <span className="text-xs font-medium text-muted-foreground">
            Measure display
          </span>
          <div
            className="flex flex-wrap gap-2"
            role="group"
            aria-label="Measure display options"
          >
            <button
              type="button"
              aria-pressed={showKeys}
              onClick={() => setShowKeys((value) => !value)}
              className="min-h-8 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring aria-pressed:bg-accent"
            >
              Exact keys
            </button>
            <button
              type="button"
              aria-pressed={showExact}
              onClick={() => setShowExact((value) => !value)}
              className="min-h-8 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring aria-pressed:bg-accent"
            >
              Exact values
            </button>
            <div className="flex" role="group" aria-label="Rows">
              <button
                type="button"
                aria-pressed={!hideZeroRows}
                onClick={() => setHideZeroRows(false)}
                className="min-h-8 rounded-l-md border border-border px-2 py-1 text-xs hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring aria-pressed:bg-accent"
              >
                Show all
              </button>
              <button
                type="button"
                aria-pressed={hideZeroRows}
                onClick={() => setHideZeroRows(true)}
                className="min-h-8 rounded-r-md border border-l-0 border-border px-2 py-1 text-xs hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring aria-pressed:bg-accent"
              >
                Nonzero only
              </button>
            </div>
          </div>
        </div>
      )}
      {grouped.length > 0 && (
        <p className="text-xs text-muted-foreground">
          {props.compareTo === undefined ? "" : "— Not supplied · "}
          {noUnits
            ? "Raw measure values · units unavailable"
            : "Units shown when supplied"}
          {showExact ? "" : " · Values shortened to six significant digits"}
        </p>
      )}
      {grouped.length ? (
        grouped.map(({ group, rows }, groupIndex) => (
          <section
            key={group}
            className="finstack-measures__group min-w-0 space-y-2"
            aria-label={`${group} measures`}
          >
            <h3 className="border-b border-border bg-muted/40 px-2 py-2 text-sm font-semibold">
              {group}
            </h3>
            {displaySections(rows, props).map((section, sectionIndex) => {
              const id = `${groupIndex}-${sectionIndex}`;
              const stateKey = `${group}:${section.id}`;
              const isExpanded = expanded[stateKey] ?? section.rows.length <= 6;
              const visibleRows = hideZeroRows
                ? section.rows.filter(
                    (row) => !isZeroOnly(row, props.compareTo !== undefined),
                  )
                : section.rows;
              const hiddenCount = section.rows.length - visibleRows.length;
              return (
                <div key={section.id} className="min-w-0">
                  {section.bucketed && (
                    <button
                      type="button"
                      aria-expanded={isExpanded}
                      aria-controls={`measure-section-${id}`}
                      onClick={() =>
                        setExpanded((current) => ({
                          ...current,
                          [stateKey]: !isExpanded,
                        }))
                      }
                      className="flex w-full items-center justify-between gap-2 rounded-md px-2 py-2 text-left text-sm font-medium hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring print:hidden"
                    >
                      <span>{section.title}</span>
                      <span className="shrink-0 text-xs font-normal text-muted-foreground">
                        {visibleRows.length}{" "}
                        {visibleRows.length === 1 ? "bucket" : "buckets"} ·{" "}
                        {isExpanded ? "Collapse" : "Expand"}
                      </span>
                    </button>
                  )}
                  <div
                    id={`measure-section-${id}`}
                    className={
                      section.bucketed && !isExpanded
                        ? "hidden print:block"
                        : ""
                    }
                  >
                    {section.bucketed && (
                      <h4 className="hidden px-2 py-2 text-sm font-medium print:block">
                        {section.title}
                      </h4>
                    )}
                    {visibleRows.length > 0 ? (
                      <FinstackTable
                        data={visibleRows}
                        columns={columns(section.bucketed)}
                        getRowId={(row) => row.key}
                        caption={
                          section.bucketed
                            ? `${group}: ${section.title}`
                            : `${group} measures`
                        }
                        captionVisibility="sr-only"
                      />
                    ) : (
                      <p className="px-2 py-2 text-xs text-muted-foreground">
                        All zero rows hidden. Select Show all to restore them.
                      </p>
                    )}
                    {hiddenCount > 0 && visibleRows.length > 0 && (
                      <p className="px-2 py-1 text-xs text-muted-foreground">
                        {hiddenCount} zero {hiddenCount === 1 ? "row" : "rows"}{" "}
                        hidden
                      </p>
                    )}
                  </div>
                </div>
              );
            })}
          </section>
        ))
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          No measures supplied
        </p>
      )}
    </div>
  );
}
