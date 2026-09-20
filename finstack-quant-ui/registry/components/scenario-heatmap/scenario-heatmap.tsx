"use client";
import { useMemo, type Ref } from "react";
import type { ScenarioTable, TrancheScenarioCell } from "finstack-quant-wasm";
import type { ChartKey, ChartMark } from "@tanstack/charts";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import type { LinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import {
  FinstackChart,
  heatmap,
  type FigureHandle,
  type FigureInteractions,
} from "../../primitives/finstack-chart/finstack-chart";
import type { FigureText } from "../../primitives/finstack-chart/presentation";
import { EnumField } from "../../primitives/enum-field/enum-field";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
/** Canonical Rust ScenarioCell.price convention; the facade's original-balance prose is outdated. */
export const scenarioPriceLabel =
  "Clean settlement price (% of current tranche balance)";
export const scenarioKey = (trancheId: string, cell: TrancheScenarioCell) =>
  JSON.stringify([trancheId, cell.cpr, cell.cdr, cell.severity]);
export interface ScenarioHeatmapProps
  extends
    FigureText,
    FigureInteractions<TrancheScenarioCell, ChartKey, ChartKey> {
  table: ScenarioTable;
  /** Select an exact returned severity, expressed as a decimal; no repricing is triggered. */
  severity: number;
  onSeverityChange(severity: number): void;
  /** Explicit price extent straddling 100, the canonical par reference. */
  priceDomain: readonly [number, number];
  link?: LinkedSelection;
  width?: number;
  height?: number;
  annotations?: readonly ChartMark<TrancheScenarioCell, ChartKey, ChartKey>[];
  figureRef?: Ref<FigureHandle>;
  formatValue?(price: number): string;
}
/** A view of original cells in input order, never a regenerated or normalized grid. */
export function scenarioCells(table: ScenarioTable, severity: number) {
  return table.cells.filter((cell) => cell.severity === severity);
}
/** Shared heatmap uses returned prices directly and the canonical par reference as its color center. */
export function scenarioPreset(
  props: Pick<
    ScenarioHeatmapProps,
    | "table"
    | "severity"
    | "priceDomain"
    | "link"
    | "annotations"
    | "formatValue"
  >,
) {
  return heatmap({
    data: scenarioCells(props.table, props.severity),
    x: (cell) => cell.cpr,
    y: (cell) => cell.cdr,
    value: (cell) => cell.price,
    key: (cell) => scenarioKey(props.table.tranche_id, cell),
    xLabel: "CPR (annual decimal)",
    yLabel: "CDR (annual decimal)",
    valueLabel: scenarioPriceLabel,
    palette: "diverging",
    domain: [props.priceDomain[0], 100, props.priceDomain[1]],
    link: props.link,
    annotations: props.annotations,
    formatValue: props.formatValue,
  });
}
/** Returned scenario prices with a controlled severity selector and complete-figure exports. No WASM is loaded here. */
export function ScenarioHeatmap(props: ScenarioHeatmapProps) {
  const { table, severity } = props;
  const severities = useMemo(
    () => [...new Set(table.cells.map((cell) => cell.severity))],
    [table],
  );
  const cells = useMemo(
    () => scenarioCells(table, severity),
    [table, severity],
  );
  const preset = useMemo(
    () => (cells.length ? scenarioPreset(props) : null),
    [
      table,
      severity,
      props.priceDomain,
      props.link,
      props.annotations,
      props.formatValue,
      cells.length,
    ],
  );
  return (
    <section
      aria-label={`${table.tranche_id} scenario prices`}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <EnumField
        label="Loss severity (decimal)"
        value={String(severity)}
        onValueChange={(value) => props.onSeverityChange(Number(value))}
        options={severities.map((value) => ({
          value: String(value),
          label: String(value),
        }))}
        disabled={!severities.length}
      />
      <p>
        {scenarioPriceLabel}. Par reference: 100. Severity: {String(severity)}.
      </p>
      {preset ? (
        <FinstackChart
          key={serializeHost([table, severity])}
          {...preset}
          ariaLabel={`${table.tranche_id} scenario heatmap`}
          ariaDescription={`${scenarioPriceLabel}; par reference 100; loss severity ${severity}`}
          title={props.title ?? `${table.tranche_id} scenario prices`}
          subtitle={
            props.subtitle ??
            `Loss severity ${severity} (decimal) · Par reference 100`
          }
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
        caption={`${table.tranche_id} selected scenario prices`}
        data={cells}
        getRowId={(cell) => scenarioKey(table.tranche_id, cell)}
        link={props.link}
        getRowKey={(cell) => scenarioKey(table.tranche_id, cell)}
        columns={[
          {
            id: "cpr",
            header: "CPR (annual decimal)",
            accessorFn: (c) => c.cpr,
          },
          {
            id: "cdr",
            header: "CDR (annual decimal)",
            accessorFn: (c) => c.cdr,
          },
          {
            id: "severity",
            header: "Loss severity (decimal)",
            accessorFn: (c) => c.severity,
          },
          {
            id: "price",
            header: scenarioPriceLabel,
            accessorFn: (c) => c.price,
          },
        ]}
      />
      <JsonViewer
        label={`${table.tranche_id} complete native scenario table`}
        text={serializeHost(table)}
      />
    </section>
  );
}
