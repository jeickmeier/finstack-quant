"use client";
import { useMemo, useState, type Ref } from "react";
import type {
  MetricFieldUnit,
  ScenarioTable,
  ScenarioCell,
  ScenarioTableFieldUnits,
} from "finstack-quant-wasm";
import type { ChartKey, ChartMark } from "@tanstack/charts";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import {
  useLinkedSelection,
  type LinkedSelection,
} from "@/hooks/shared/use-linked-selection/use-linked-selection";
import {
  FinstackChart,
  heatmap,
  type FigureHandle,
  type FigureInteractions,
} from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import type { FigureText } from "@/components/finstack/shared/chart/finstack-chart/presentation";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
export const scenarioKey = (trancheId: string, cell: ScenarioCell) =>
  JSON.stringify([trancheId, cell.cpr, cell.cdr, cell.severity]);
/** A cell field with the unit Rust states for it. */
const fieldLabel = (unit: MetricFieldUnit) =>
  `${labelFor(unit.field)} (${unit.label})`;
/** The numeric cell field Rust's unit record names. */
const cellField = (cell: ScenarioCell, unit: MetricFieldUnit) =>
  cell[unit.field as keyof ScenarioCell];
export interface ScenarioHeatmapProps
  extends FigureText, FigureInteractions<ScenarioCell, ChartKey, ChartKey> {
  table: ScenarioTable;
  /** Native scenarioTableFieldUnits for the table's currency: the axes, and the measures offered in their order. */
  units: ScenarioTableFieldUnits;
  /** Select an exact returned severity, expressed as a decimal; no repricing is triggered. */
  severity: number;
  onSeverityChange(severity: number): void;
  /** The `units.measures` field the heatmap colours. Switching reads the same cells; no repricing is triggered. */
  measure: string;
  onMeasureChange(measure: string): void;
  /** Extent of the price measure, straddling 100, the canonical par reference; omitted means the returned prices' reach about par. */
  priceDomain?: readonly [number, number];
  link?: LinkedSelection;
  width?: number;
  height?: number;
  annotations?: readonly ChartMark<ScenarioCell, ChartKey, ChartKey>[];
  figureRef?: Ref<FigureHandle>;
  formatValue?(value: number): string;
}
/** A view of original cells in input order, never a regenerated or normalized grid. */
export function scenarioCells(table: ScenarioTable, severity: number) {
  return table.cells.filter((cell) => cell.severity === severity);
}
/** Color extent of the table's prices, symmetric about par so the ramp's center means par. */
function parDomain(table: ScenarioTable): readonly [number, number] {
  // finstack-arith: chart color domain about the par reference; no value is changed
  const reach = Math.max(1, ...table.cells.map((c) => Math.abs(c.price - 100)));
  // finstack-arith: chart color domain about the par reference; no value is changed
  return [100 - reach, 100 + reach];
}
/** Color extent of a measure over the whole table, so a severity switch keeps the scale. */
function measureDomain(
  table: ScenarioTable,
  unit: MetricFieldUnit,
): readonly [number, number] {
  const values = table.cells.map((cell) => cellField(cell, unit));
  // finstack-arith: chart color domain over the returned values; no value is changed
  const low = Math.min(...values);
  // finstack-arith: chart color domain over the returned values; no value is changed
  const high = Math.max(...values);
  // finstack-arith: a flat measure still needs an increasing chart color domain; no value is changed
  return high > low ? [low, high] : [low, low + 1];
}
/** Shared heatmap of one returned measure: price centers its color on the canonical par reference, the others run low to high. */
export function scenarioPreset(
  props: Pick<
    ScenarioHeatmapProps,
    | "table"
    | "units"
    | "measure"
    | "severity"
    | "priceDomain"
    | "link"
    | "annotations"
    | "formatValue"
  >,
) {
  const { table, units } = props;
  const unit = units.measures.find((entry) => entry.field === props.measure)!;
  const [x, y] = units.axes;
  const shared = {
    data: scenarioCells(table, props.severity),
    x: (cell: ScenarioCell) => cell.cpr,
    y: (cell: ScenarioCell) => cell.cdr,
    value: (cell: ScenarioCell) => cellField(cell, unit),
    key: (cell: ScenarioCell) => scenarioKey(table.tranche_id, cell),
    xLabel: fieldLabel(x!),
    yLabel: fieldLabel(y!),
    valueLabel: fieldLabel(unit),
    link: props.link,
    annotations: props.annotations,
    formatValue: props.formatValue,
  };
  if (props.measure !== "price")
    return heatmap({
      ...shared,
      palette: "sequential",
      domain: measureDomain(table, unit),
    });
  const [low, high] = props.priceDomain ?? parDomain(table);
  return heatmap({ ...shared, palette: "diverging", domain: [low, 100, high] });
}
/** Returned scenario measures with controlled severity and measure selectors and complete-figure exports. No WASM is loaded here. */
export function ScenarioHeatmap(props: ScenarioHeatmapProps) {
  const { table, units, severity, measure } = props;
  const severities = useMemo(
    () => [...new Set(table.cells.map((cell) => cell.severity))],
    [table],
  );
  const cells = useMemo(
    () => scenarioCells(table, severity),
    [table, severity],
  );
  const unit = units.measures.find((entry) => entry.field === measure);
  const preset = useMemo(
    () => (cells.length && unit ? scenarioPreset(props) : null),
    [
      table,
      units,
      measure,
      severity,
      props.priceDomain,
      props.link,
      props.annotations,
      props.formatValue,
      cells.length,
      unit,
    ],
  );
  const severityUnit = units.axes.find((entry) => entry.field === "severity");
  const label = unit ? fieldLabel(unit) : measure;
  const par = measure === "price" ? "Par reference: 100. " : "";
  return (
    <section
      aria-label={`${table.tranche_id} scenario measures`}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <div className="flex flex-wrap items-start gap-6">
        <EnumField
          label="Measure"
          value={measure}
          onValueChange={props.onMeasureChange}
          options={units.measures.map((entry) => ({
            value: entry.field,
            label: labelFor(entry.field),
          }))}
          layout="radio"
        />
        <EnumField
          label={severityUnit ? fieldLabel(severityUnit) : "Severity"}
          value={String(severity)}
          onValueChange={(value) => props.onSeverityChange(Number(value))}
          options={severities.map((value) => ({
            value: String(value),
            label: String(value),
          }))}
          disabled={!severities.length}
        />
      </div>
      <p>
        {label}. {par}Severity: {String(severity)}.
      </p>
      {preset ? (
        <FinstackChart
          key={serializeHost([table, severity, measure])}
          {...preset}
          ariaLabel={`${table.tranche_id} scenario heatmap`}
          ariaDescription={`${label}; ${par}severity ${severity}`}
          title={
            props.title ?? `${table.tranche_id} scenario ${labelFor(measure)}`
          }
          subtitle={props.subtitle ?? `${par}Severity ${severity}`}
          caption={props.caption}
          sources={props.sources}
          figureAnnotations={props.figureAnnotations}
          width={props.width}
          height={props.height}
          ref={props.figureRef}
          onSelect={props.onSelect}
          onFocusChange={props.onFocusChange}
          onFocusGroupChange={props.onFocusGroupChange}
          renderTooltipBody={props.renderTooltipBody}
        />
      ) : (
        <p>
          {table.cells.length
            ? "Selected severity unavailable"
            : "No returned scenario cells"}
        </p>
      )}
      <FinstackTable
        caption={`${table.tranche_id} selected scenario measures`}
        data={cells}
        getRowId={(cell) => scenarioKey(table.tranche_id, cell)}
        link={props.link}
        getRowKey={(cell) => scenarioKey(table.tranche_id, cell)}
        columns={[...units.axes, ...units.measures].map((entry) => ({
          id: entry.field,
          header: fieldLabel(entry),
          accessorFn: (cell: ScenarioCell) => cellField(cell, entry),
        }))}
      />
      <JsonViewer
        label={`${table.tranche_id} complete native scenario table`}
        text={serializeHost(table)}
      />
    </section>
  );
}
/** Self-contained supplied-result view with owned severity, measure and selection. Remount when loading a different table. */
export function ScenarioHeatmapPanel({
  table,
  units,
  priceDomain,
}: {
  table: ScenarioTable;
  units: ScenarioTableFieldUnits;
  priceDomain?: readonly [number, number];
}) {
  const [severity, setSeverity] = useState(table.cells[0]?.severity),
    [measure, setMeasure] = useState(units.measures[0]!.field),
    link = useLinkedSelection();
  if (severity === undefined) return <p>No returned scenario cells</p>;
  return (
    <ScenarioHeatmap
      table={table}
      units={units}
      severity={severity}
      onSeverityChange={setSeverity}
      measure={measure}
      onMeasureChange={setMeasure}
      priceDomain={priceDomain}
      link={link}
      caption="Original native scenario table"
      sources={[{ label: "trancheScenarioTable" }]}
    />
  );
}
