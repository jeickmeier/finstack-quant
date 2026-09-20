"use client";
import { useMemo, type Ref } from "react";
import type { CalibrationReport } from "finstack-quant-wasm";
import type { CalibrationWire } from "@/lib/finstack/generated/types/calibration";
import { defineChart, dot, ruleY, type ChartMark } from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { decorative } from "@tanstack/charts/mark/decorative";
import { whenSelected } from "@tanstack/charts/selection";
import { tooltip } from "@tanstack/charts/tooltip";
import {
  FinstackChart,
  chartSelection,
  type FigureHandle,
  type FigureInteractions,
} from "../../primitives/finstack-chart/finstack-chart";
import type { FigureText } from "../../primitives/finstack-chart/presentation";
import type { LinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import { residualPanels, type ResidualPoint } from "./residuals";
export { residualPanels, residualKey, type ResidualPoint } from "./residuals";
export interface CalibrationFitChartProps
  extends FigureText, FigureInteractions<ResidualPoint, string, number> {
  stepId: string;
  report: CalibrationReport;
  /** Exact original inputs classify unlike quote kinds into separate panels. Unmatched quotes are isolated. */
  marketData?: CalibrationWire["market_data"];
  link?: LinkedSelection;
  width?: number;
  height?: number;
  /** Native marks in original quote/residual coordinates, keyed by residualPanels().key. */
  annotations?: Record<
    string,
    readonly ChartMark<ResidualPoint, string, number>[]
  >;
  figureRefs?: Record<string, Ref<FigureHandle>>;
}
/** Native signed residual points and a decorative zero rule; no repricing or residual conversion. */
export function residualDefinition(
  panel: ReturnType<typeof residualPanels>[number],
  props: Pick<CalibrationFitChartProps, "link" | "annotations">,
) {
  const selection = props.link
    ? chartSelection<ResidualPoint, string, number>(props.link, (p) => p.key)
    : undefined;
  const channels = {
    x: (p: ResidualPoint) => p.quoteId,
    y: (p: ResidualPoint) => p.residual,
    key: (p: ResidualPoint) => p.key,
    r: 5,
  };
  return defineChart({
    marks: [
      decorative(
        ruleY([0], {
          id: "zero-target",
          stroke: "var(--border)",
          strokeDasharray: "3 3",
        }),
      ),
      dot(panel.points, { ...channels, id: "returned-residuals" }),
      ...(selection
        ? [
            whenSelected(
              dot(panel.points, {
                ...channels,
                id: "accepted",
                r: 9,
                fill: "none",
                stroke: "var(--primary)",
                strokeWidth: 2,
              }),
              selection,
            ),
          ]
        : []),
      ...(props.annotations?.[panel.key] ?? []),
    ],
    scales: {
      x: { scale: scaleBand, axis: { label: "Original quote identity" } },
      y: {
        scale: scaleLinear,
        nice: true,
        axis: {
          label: panel.units,
          ticks: { format: (value) => String(value) },
        },
      },
    },
    selection,
    tooltip: {
      use: tooltip,
      content: (points) => ({
        rows: points.map((point) => ({
          label: `${point.datum.stepId} · ${point.datum.quoteId}`,
          value: `Residual ${point.datum.residual} · ${panel.units} · Target 0`,
        })),
      }),
    },
  });
}
function Panel({
  panel,
  props,
}: {
  panel: ReturnType<typeof residualPanels>[number];
  props: CalibrationFitChartProps;
}) {
  const definition = useMemo(
    () => residualDefinition(panel, props),
    [panel, props.link, props.annotations],
  );
  return (
    <FinstackChart
      key={JSON.stringify([panel.key, props.report])}
      definition={definition}
      ariaLabel={`${panel.stepId} ${panel.group} residuals`}
      ariaDescription="Returned solver residuals against zero; not observed versus repriced quotes"
      title={props.title ?? `${panel.stepId} · ${panel.group}`}
      subtitle={props.subtitle ?? "Returned residuals against zero"}
      caption={props.caption}
      sources={props.sources}
      figureAnnotations={props.figureAnnotations}
      width={props.width}
      height={props.height}
      ref={props.figureRefs?.[panel.key]}
      onSelect={props.onSelect}
      onFocusChange={props.onFocusChange}
      onFocusGroupChange={props.onFocusGroupChange}
      renderTooltipBody={props.renderTooltipBody}
    />
  );
}
/** Independently exported figures per step/quote kind. Quote-space fit data remains unavailable. */
export function CalibrationFitChart(props: CalibrationFitChartProps) {
  const panels = useMemo(
    () => residualPanels(props.stepId, props.report, props.marketData),
    [props.stepId, props.report, props.marketData],
  );
  return (
    <section
      aria-label={`${props.stepId} calibration residuals`}
      className="space-y-4"
    >
      {panels.length ? (
        panels.map((panel) => (
          <Panel key={panel.key} panel={panel} props={props} />
        ))
      ) : (
        <p>No returned residuals</p>
      )}
    </section>
  );
}
