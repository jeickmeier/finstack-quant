"use client";

import { useId, useState } from "react";
import type { MetricMetadata, ValuationResult } from "finstack-quant-wasm";
import { MeasureValue } from "../../primitives/measure-value/measure-value";

type MeasureResult = Pick<ValuationResult, "measures">;

export interface DimensionalMeasuresProps {
  /** Returned native measures; values are never calculated or filled in by this view. */
  result?: MeasureResult | null;
  /** Native interpretation of the primary result's complete measure keys. */
  metadata?: readonly MetricMetadata[];
  /** Independent result to display beside the primary valuation. */
  compareTo?: MeasureResult | null;
  /** Native interpretation of the comparison result's complete measure keys. */
  comparisonMetadata?: readonly MetricMetadata[];
}

type MeasureEntry = {
  key: string;
  metric: string;
  group: string | null;
  coordinates: readonly string[];
  unit: MetricMetadata["unit"];
  comparisonUnit: MetricMetadata["unit"] | undefined;
  bucketed: boolean;
  value: number | undefined;
  comparison: number | undefined;
};

const metadataSource = "finstack_quant_valuations::pricer::metric_metadata";

function dimensionalEntries(props: DimensionalMeasuresProps): MeasureEntry[] {
  const primary = new Map(props.metadata?.map((entry) => [entry.key, entry]));
  const comparison = new Map(
    props.comparisonMetadata?.map((entry) => [entry.key, entry]),
  );
  const keys = new Set([
    ...Object.keys(props.result?.measures ?? {}),
    ...Object.keys(props.compareTo?.measures ?? {}),
  ]);

  return [...keys].flatMap((key) => {
    const descriptor = primary.get(key) ?? comparison.get(key);
    if (!descriptor || descriptor.components.length === 0) return [];
    return [
      {
        key,
        metric: descriptor.metric,
        group: descriptor.group,
        coordinates: descriptor.components,
        unit: primary.get(key)?.unit ?? descriptor.unit,
        comparisonUnit: comparison.get(key)?.unit,
        bucketed: descriptor.bucketed,
        value: props.result?.measures[key],
        comparison: props.compareTo?.measures[key],
      },
    ];
  });
}

function unclassifiedEntries(props: DimensionalMeasuresProps) {
  const described = new Set([
    ...(props.metadata?.map((entry) => entry.key) ?? []),
    ...(props.comparisonMetadata?.map((entry) => entry.key) ?? []),
  ]);
  const keys = new Set([
    ...Object.keys(props.result?.measures ?? {}),
    ...Object.keys(props.compareTo?.measures ?? {}),
  ]);
  return [...keys]
    .filter((key) => !described.has(key))
    .map((key) => ({
      key,
      value: props.result?.measures[key],
      comparison: props.compareTo?.measures[key],
    }));
}

/** Count returned keys with coordinates in native metric metadata. */
export function dimensionalMeasureCount(
  props: DimensionalMeasuresProps,
): number {
  return dimensionalEntries(props).length;
}

function metricLabel(metric: string): string {
  return metric
    .split("_")
    .map((part) =>
      /^(dv01|cs01|fx01|pv01|var|oas)$/i.test(part)
        ? part.toUpperCase()
        : `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`,
    )
    .join(" ");
}

function shortValue(value: number | undefined): string | undefined {
  return value === undefined || !Number.isFinite(value)
    ? undefined
    : String(Number(value.toPrecision(6)));
}

function Value({
  value,
  unit,
  compact = true,
}: {
  value: number | undefined;
  unit?: MetricMetadata["unit"];
  compact?: boolean;
}) {
  return (
    <MeasureValue
      value={value}
      displayText={compact ? shortValue(value) : undefined}
      signed={value !== undefined && Number.isFinite(value)}
      unit={
        compact || !unit || unit === "unknown"
          ? undefined
          : { label: unit, source: metadataSource }
      }
      showUnavailableUnit={false}
    />
  );
}

function axisLabel(index: number, bucketed: boolean): string {
  if (bucketed && index === 0) return "Identifier";
  if (bucketed && index === 1) return "Bucket";
  return `Coordinate ${index + 1}`;
}

function coordinateOrder(left: string, right: string): number {
  const tenor = /^(\d+(?:\.\d+)?)([dwmy])$/;
  const a = tenor.exec(left);
  const b = tenor.exec(right);
  if (a && b) {
    const days = { d: 1, w: 7, m: 30, y: 365 };
    const difference =
      Number(a[1]) * days[a[2] as keyof typeof days] -
      Number(b[1]) * days[b[2] as keyof typeof days];
    if (difference) return difference;
  }
  return left.localeCompare(right, undefined, { numeric: true });
}

function Values({
  entry,
  comparing,
}: {
  entry: MeasureEntry | undefined;
  comparing: boolean;
}) {
  return (
    <div className="flex flex-col items-end gap-0.5 whitespace-nowrap font-mono text-xs tabular-nums">
      <span title={entry?.key}>
        <Value value={entry?.value} unit={entry?.unit} />
      </span>
      {comparing && (
        <span className="text-muted-foreground" title={entry?.key}>
          <span className="sr-only">Comparison: </span>
          <Value value={entry?.comparison} unit={entry?.comparisonUnit} />
        </span>
      )}
    </div>
  );
}

function CoordinateTable({
  entries,
  axisOffset,
  comparing,
  bucketed,
}: {
  entries: readonly MeasureEntry[];
  axisOffset: number;
  comparing: boolean;
  bucketed: boolean;
}) {
  const dimensions = entries[0]?.coordinates.length ?? axisOffset;
  return (
    <div className="overflow-x-auto rounded-md border border-border bg-card">
      <table className="w-full min-w-max border-collapse text-left text-xs">
        <caption className="sr-only">
          Returned coordinates and measure values
        </caption>
        <thead className="bg-muted/50 text-muted-foreground">
          <tr>
            {Array.from({ length: dimensions - axisOffset }, (_, offset) => (
              <th
                key={offset}
                scope="col"
                className="border-b border-border px-3 py-2 font-medium"
              >
                {axisLabel(axisOffset + offset, bucketed)}
              </th>
            ))}
            <th
              scope="col"
              className="border-b border-border px-3 py-2 text-right font-medium"
            >
              Valuation
            </th>
            {comparing && (
              <th
                scope="col"
                className="border-b border-border px-3 py-2 text-right font-medium"
              >
                Comparison
              </th>
            )}
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => (
            <tr
              key={entry.key}
              className="border-t border-border/60 first:border-t-0"
            >
              {entry.coordinates.slice(axisOffset).map((coordinate, index) =>
                index === 0 ? (
                  <th
                    key={index}
                    scope="row"
                    title={entry.key}
                    className="max-w-56 px-3 py-1.5 font-mono font-normal text-foreground"
                  >
                    <span className="block truncate">{coordinate || "—"}</span>
                  </th>
                ) : (
                  <td
                    key={index}
                    title={entry.key}
                    className="max-w-56 px-3 py-1.5 font-mono text-foreground"
                  >
                    <span className="block truncate">{coordinate || "—"}</span>
                  </td>
                ),
              )}
              <td className="px-3 py-1.5 text-right font-mono tabular-nums">
                <Value value={entry.value} />
              </td>
              {comparing && (
                <td className="px-3 py-1.5 text-right font-mono tabular-nums text-muted-foreground">
                  <Value value={entry.comparison} />
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function CoordinateView({
  entries,
  axisOffset,
  comparing,
  bucketed,
}: {
  entries: readonly MeasureEntry[];
  axisOffset: number;
  comparing: boolean;
  bucketed: boolean;
}) {
  const axes = entries[0]?.coordinates.length ?? 0;
  if (axes - axisOffset !== 2) {
    return (
      <CoordinateTable
        entries={entries}
        axisOffset={axisOffset}
        comparing={comparing}
        bucketed={bucketed}
      />
    );
  }

  const rows = [
    ...new Set(entries.map((entry) => entry.coordinates[axisOffset])),
  ].sort(coordinateOrder);
  const columns = [
    ...new Set(entries.map((entry) => entry.coordinates[axisOffset + 1])),
  ].sort(coordinateOrder);
  const cells = new Map<string, MeasureEntry>();
  let regular = rows.every(Boolean) && columns.every(Boolean);
  for (const entry of entries) {
    const coordinate = JSON.stringify(entry.coordinates.slice(axisOffset));
    if (cells.has(coordinate)) regular = false;
    cells.set(coordinate, entry);
  }
  regular &&= entries.length === rows.length * columns.length;
  if (!regular) {
    return (
      <CoordinateTable
        entries={entries}
        axisOffset={axisOffset}
        comparing={comparing}
        bucketed={bucketed}
      />
    );
  }

  if (rows.length === 1 || columns.length === 1) {
    const fixedAxis = rows.length === 1 ? axisOffset : axisOffset + 1;
    const fixed = rows.length === 1 ? rows[0] : columns[0];
    const labels = rows.length === 1 ? columns : rows;
    const variableAxis = rows.length === 1 ? axisOffset + 1 : axisOffset;
    return (
      <div className="overflow-hidden rounded-md border border-border bg-card">
        <div className="flex items-center gap-2 border-b border-border bg-muted/40 px-3 py-1.5 text-xs">
          <span className="text-muted-foreground">
            {axisLabel(fixedAxis, bucketed)}
          </span>
          <span className="font-mono font-medium text-foreground">{fixed}</span>
        </div>
        <table className="w-full border-collapse text-xs">
          <caption className="sr-only">
            Returned bucket values for {fixed}
          </caption>
          <thead className="text-muted-foreground">
            <tr>
              <th scope="col" className="px-3 py-1.5 text-left font-medium">
                {axisLabel(variableAxis, bucketed)}
              </th>
              <th scope="col" className="px-3 py-1.5 text-right font-medium">
                {comparing ? "Valuation / comparison" : "Valuation"}
              </th>
            </tr>
          </thead>
          <tbody>
            {labels.map((label) => {
              const coordinates =
                rows.length === 1 ? [fixed, label] : [label, fixed];
              const entry = cells.get(JSON.stringify(coordinates));
              return (
                <tr key={label} className="border-t border-border/60">
                  <th
                    scope="row"
                    className="px-3 py-1.5 text-left font-mono font-normal text-foreground"
                  >
                    {label}
                  </th>
                  <td className="px-3 py-1.5 text-right">
                    <Values entry={entry} comparing={comparing} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    );
  }

  return (
    <div className="overflow-x-auto rounded-md border border-border bg-card">
      <table className="w-full min-w-max border-collapse text-xs">
        <caption className="sr-only">
          Returned two-coordinate measure matrix
        </caption>
        <thead className="bg-muted/40">
          <tr>
            <th
              scope="col"
              className="sticky left-0 z-10 min-w-28 border-b border-r border-border bg-muted px-3 py-2 text-left font-medium text-muted-foreground"
            >
              {axisLabel(axisOffset, bucketed)} /{" "}
              {axisLabel(axisOffset + 1, bucketed)}
            </th>
            {columns.map((column) => (
              <th
                key={column}
                scope="col"
                className="min-w-20 border-b border-border px-3 py-2 text-right font-mono font-medium text-foreground"
              >
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row} className="border-t border-border/60">
              <th
                scope="row"
                className="sticky left-0 z-10 border-r border-border bg-card px-3 py-2 text-left font-mono font-medium text-foreground"
              >
                {row}
              </th>
              {columns.map((column) => (
                <td key={column} className="px-3 py-2 text-right">
                  <Values
                    entry={cells.get(JSON.stringify([row, column]))}
                    comparing={comparing}
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function MetricSection({
  entries,
  comparing,
}: {
  entries: readonly MeasureEntry[];
  comparing: boolean;
}) {
  const id = useId();
  const [slice, setSlice] = useState<string>();
  const first = entries[0];
  if (!first) return null;
  const threeDimensional = first.coordinates.length === 3;
  const slices = threeDimensional
    ? [...new Set(entries.map((entry) => entry.coordinates[0]))].sort(
        coordinateOrder,
      )
    : [];
  const activeSlice = slice && slices.includes(slice) ? slice : slices[0];
  const visible = threeDimensional
    ? entries.filter((entry) => entry.coordinates[0] === activeSlice)
    : entries;
  const comparisonUnits = [
    ...new Set(entries.map((entry) => entry.comparisonUnit).filter(Boolean)),
  ];
  const bucketed = entries.every((entry) => entry.bucketed);

  return (
    <section className="min-w-0 border-t border-border pt-3 first:border-t-0 first:pt-0">
      <div className="mb-2 flex flex-wrap items-start justify-between gap-x-4 gap-y-1">
        <div className="min-w-0">
          <p className="text-[10px] font-semibold uppercase tracking-[0.11em] text-primary">
            {first.group ?? "Other measures"}
          </p>
          <h3 className="text-sm font-semibold leading-5 text-foreground">
            {metricLabel(first.metric)}
          </h3>
        </div>
        <div className="flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
          <span className="rounded border border-border bg-muted/40 px-1.5 py-0.5">
            {entries.length} {entries.length === 1 ? "value" : "values"}
          </span>
          <span className="rounded border border-border bg-muted/40 px-1.5 py-0.5">
            {first.coordinates.length} axes
          </span>
          <span className="rounded border border-border bg-muted/40 px-1.5 py-0.5">
            {first.unit === "unknown" ? "Unit unavailable" : first.unit}
          </span>
        </div>
      </div>

      {comparing && (
        <p className="mb-2 text-[11px] text-muted-foreground">
          Values are valuation first, comparison second.
          {comparisonUnits.length === 0
            ? " Comparison unit unavailable."
            : comparisonUnits.length > 1
              ? " Comparison units vary; see exact keys."
              : comparisonUnits[0] !== first.unit
                ? ` Comparison unit: ${comparisonUnits[0]}.`
                : ""}
        </p>
      )}

      {threeDimensional && (
        <div className="mb-2 flex flex-wrap items-center gap-2 text-xs">
          <label htmlFor={id} className="font-medium text-muted-foreground">
            First returned coordinate
          </label>
          <select
            id={id}
            value={activeSlice}
            onChange={(event) => setSlice(event.target.value)}
            className="min-h-8 max-w-full rounded-md border border-border bg-card px-2 font-mono text-xs text-foreground focus-visible:outline-2 focus-visible:outline-ring"
          >
            {slices.map((option) => (
              <option key={option} value={option}>
                {option || "—"}
              </option>
            ))}
          </select>
          <span className="text-muted-foreground">
            {visible.length} {visible.length === 1 ? "value" : "values"} in
            slice
          </span>
        </div>
      )}

      <CoordinateView
        entries={visible}
        axisOffset={threeDimensional ? 1 : 0}
        comparing={comparing}
        bucketed={bucketed}
      />

      <details className="mt-2 text-xs text-muted-foreground">
        <summary className="w-fit cursor-pointer rounded-sm py-1 hover:text-foreground focus-visible:outline-2 focus-visible:outline-ring">
          Exact {metricLabel(first.metric)} keys and values
        </summary>
        <div className="max-h-52 overflow-auto rounded-md border border-border bg-muted/20">
          <table className="w-full border-collapse text-left text-xs">
            <caption className="sr-only">
              Complete returned metric keys and exact values
            </caption>
            <thead>
              <tr className="border-b border-border">
                <th scope="col" className="px-2 py-1.5 font-medium">
                  Canonical key
                </th>
                <th scope="col" className="px-2 py-1.5 text-right font-medium">
                  Valuation
                </th>
                {comparing && (
                  <th
                    scope="col"
                    className="px-2 py-1.5 text-right font-medium"
                  >
                    Comparison
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => (
                <tr
                  key={entry.key}
                  className="border-t border-border/60 first:border-t-0"
                >
                  <th
                    scope="row"
                    className="max-w-72 break-all px-2 py-1.5 font-mono font-normal text-foreground"
                  >
                    {entry.key}
                  </th>
                  <td className="px-2 py-1.5 text-right font-mono tabular-nums">
                    <Value
                      value={entry.value}
                      unit={entry.unit}
                      compact={false}
                    />
                  </td>
                  {comparing && (
                    <td className="px-2 py-1.5 text-right font-mono tabular-nums">
                      <Value
                        value={entry.comparison}
                        unit={entry.comparisonUnit}
                        compact={false}
                      />
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </section>
  );
}

/** Compact, source-backed presentation for returned measures with coordinates. */
export function DimensionalMeasures(props: DimensionalMeasuresProps) {
  const entries = dimensionalEntries(props);
  const unclassified = unclassifiedEntries(props);
  if (!entries.length && !unclassified.length) {
    return (
      <p role="status" className="px-1 py-4 text-sm text-muted-foreground">
        No coordinate measures returned. Request a bucketed or qualified metric
        to populate this view.
      </p>
    );
  }
  const sections = new Map<string, MeasureEntry[]>();
  for (const entry of entries) {
    const id = JSON.stringify([
      entry.group,
      entry.metric,
      entry.coordinates.length,
      entry.unit,
      entry.bucketed,
    ]);
    const section = sections.get(id) ?? [];
    section.push(entry);
    sections.set(id, section);
  }

  return (
    <div
      className="min-w-0 space-y-3 font-sans [&_.finstack-numeric]:font-mono"
      aria-label="Bucketed and multi-axis measures"
    >
      {entries.length > 0 && (
        <p className="text-[11px] text-muted-foreground">
          Native coordinates and units · rounded display, exact values below
          each section
        </p>
      )}
      {[...sections].map(([id, section]) => (
        <MetricSection
          key={id}
          entries={section}
          comparing={props.compareTo !== undefined && props.compareTo !== null}
        />
      ))}
      {unclassified.length > 0 && (
        <section className="border-t border-border pt-3 first:border-t-0 first:pt-0">
          <h3 className="text-sm font-semibold text-foreground">
            Metadata unavailable
          </h3>
          <p className="mb-2 text-xs text-muted-foreground">
            These returned keys have no native coordinate or unit metadata, so
            they remain unclassified.
          </p>
          <div className="overflow-x-auto rounded-md border border-border bg-card">
            <table className="w-full min-w-max border-collapse text-xs">
              <caption className="sr-only">
                Returned values without native metadata
              </caption>
              <thead className="bg-muted/40 text-muted-foreground">
                <tr>
                  <th scope="col" className="px-3 py-2 text-left font-medium">
                    Canonical key
                  </th>
                  <th scope="col" className="px-3 py-2 text-right font-medium">
                    Valuation
                  </th>
                  {props.compareTo && (
                    <th
                      scope="col"
                      className="px-3 py-2 text-right font-medium"
                    >
                      Comparison
                    </th>
                  )}
                </tr>
              </thead>
              <tbody>
                {unclassified.map((entry) => (
                  <tr key={entry.key} className="border-t border-border/60">
                    <th
                      scope="row"
                      className="max-w-72 break-all px-3 py-1.5 text-left font-mono font-normal text-foreground"
                    >
                      {entry.key}
                    </th>
                    <td className="px-3 py-1.5 text-right font-mono tabular-nums">
                      <Value value={entry.value} compact={false} />
                    </td>
                    {props.compareTo && (
                      <td className="px-3 py-1.5 text-right font-mono tabular-nums">
                        <Value value={entry.comparison} compact={false} />
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}
