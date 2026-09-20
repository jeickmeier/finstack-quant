"use client";
import { useMemo, type Ref } from "react";
import {
  defineChart,
  dot,
  type ChartKey,
  type ChartMark,
} from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { whenSelected } from "@tanstack/charts/selection";
import {
  FinstackChart,
  heatmap,
  chartSelection,
  type FigureHandle,
  type FigureInteractions,
} from "../../primitives/finstack-chart/finstack-chart";
import type { FigureText } from "../../primitives/finstack-chart/presentation";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import {
  useLinkedSelection,
  type LinkedSelection,
} from "@/hooks/use-linked-selection/use-linked-selection";
import { surfaceSlices, type SurfacePoint } from "./stored";
export interface SurfaceViewProps<T extends SurfacePoint>
  extends FigureText, FigureInteractions<T, ChartKey, ChartKey> {
  id: string;
  mode: "stored" | "evaluated";
  nodes: T[];
  labels: { expiry: string; secondary: string; value: string };
  description: string;
  revision: string;
  /** Caller-owned shared semantic key; omit for this component to own selection. */
  link?: LinkedSelection;
  height?: number;
  width?: number;
  /** Explicit color extent in the stored quote units, including every supplied node. */
  colorDomain: readonly [number, number];
  annotations?: readonly ChartMark<T, ChartKey, ChartKey>[];
  figureRefs?: Partial<Record<"heatmap" | "row" | "column", Ref<FigureHandle>>>;
}
/** Stored nodes, table and slices sharing one accepted selection and native chart primitives. */
export function SurfaceView<T extends SurfacePoint>(
  props: SurfaceViewProps<T>,
) {
  const owned = useLinkedSelection({
    defaultSelectedKey: props.nodes[0]?.key ?? null,
  });
  const link = props.link ?? owned;
  const { nodes, labels } = props;
  const slices = surfaceSlices(nodes, link.selectedKey);
  // Removed coordinates have no active representation. The external owner retains its state.
  const accepted = { ...link, selectedKey: slices.selected?.key ?? null };
  const preset = useMemo(
    () =>
      heatmap({
        data: nodes,
        x: (node) => node.secondary,
        y: (node) => node.expiry,
        value: (node) => node.value,
        key: (node) => node.key,
        xLabel: labels.secondary,
        yLabel: labels.expiry,
        valueLabel: labels.value,
        palette: "sequential",
        domain: props.colorDomain,
        link: accepted,
        annotations: props.annotations,
      }),
    [
      nodes,
      labels.secondary,
      labels.expiry,
      labels.value,
      props.colorDomain,
      accepted.selectedKey,
      accepted.select,
      props.annotations,
    ],
  );
  const prose = {
    title: props.title ?? props.id,
    subtitle: props.subtitle,
    caption: props.caption,
    sources: props.sources,
    figureAnnotations: props.figureAnnotations,
  };
  const interactions = {
    onSelect: props.onSelect,
    onFocusChange: props.onFocusChange,
    onFocusGroupChange: props.onFocusGroupChange,
    renderTooltipBody: props.renderTooltipBody,
  };
  const coordinates = props.revision;
  return (
    <section
      aria-label={`${props.id} ${props.mode} surface`}
      className="@container space-y-3 font-sans text-sm text-foreground"
      style={{ width: props.width, maxWidth: "100%" }}
    >
      <p className="text-xs text-muted-foreground">{props.description}</p>
      <div className="grid min-w-0 gap-4 @min-[720px]:grid-cols-[minmax(0,360px)_minmax(0,1fr)]">
        <div className="min-w-0">
          {nodes.length ? (
            <FinstackChart
              key={coordinates}
              {...preset}
              {...prose}
              {...interactions}
              ariaLabel={`${props.id} heatmap`}
              height={props.height ?? 360}
              sourceDisplay="disclosure"
              ref={props.figureRefs?.heatmap}
            />
          ) : (
            <p>No nodes supplied</p>
          )}
          <div className="flex flex-wrap gap-2 text-xs">
            <label>
              {props.mode === "stored"
                ? "Stored coordinate"
                : "Evaluated coordinate"}
              <select
                aria-label={
                  props.mode === "stored"
                    ? "Stored coordinate"
                    : "Evaluated coordinate"
                }
                value={accepted.selectedKey ?? ""}
                onChange={(event) => link.select(event.target.value || null)}
                className="ml-2 max-w-full rounded-sm border border-border bg-background px-2"
              >
                <option value="">Choose a node</option>
                {nodes.map((node) => (
                  <option key={node.key} value={node.key}>
                    {labels.expiry} {node.expiry} · {labels.secondary}{" "}
                    {node.secondary}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" onClick={link.clear}>
              Clear selected node
            </button>
          </div>
        </div>
        <div className="grid min-w-0 gap-3">
          {slices.selected ? (
            (["row", "column"] as const).map((kind) => {
              const horizontal =
                kind === "row" ? labels.secondary : labels.expiry;
              const selection = chartSelection<T, number, number>(
                accepted,
                (node) => node.key,
              );
              const channels = {
                x: (node: T) => (kind === "row" ? node.secondary : node.expiry),
                y: (node: T) => node.value,
                key: (node: T) => node.key,
              };
              const definition = defineChart({
                marks: [
                  dot(slices[kind], { ...channels, r: 5 }),
                  whenSelected(
                    dot(slices[kind], {
                      ...channels,
                      r: 9,
                      fill: "none",
                      stroke: "var(--primary)",
                      strokeWidth: 2,
                    }),
                    selection,
                  ),
                ],
                scales: {
                  x: { scale: scaleLinear, axis: { label: horizontal } },
                  y: { scale: scaleLinear, axis: { label: "Value" } },
                },
                selection,
                tooltip: {
                  use: tooltip,
                  content: (points) => ({
                    rows: points.map((point) => ({
                      label: `${horizontal}: ${channels.x(point.datum)}`,
                      value: `${labels.value}: ${point.datum.value}`,
                    })),
                  }),
                },
              });
              return (
                <FinstackChart
                  key={`${coordinates}-${kind}`}
                  definition={definition}
                  {...prose}
                  {...interactions}
                  title={`${props.id} ${props.mode} ${kind} slice`}
                  subtitle={[
                    props.subtitle,
                    `${labels.value} · ${kind === "row" ? labels.expiry : labels.secondary} ${kind === "row" ? slices.selected!.expiry : slices.selected!.secondary}`,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                  ariaLabel={`${props.id} ${kind} slice`}
                  height={240}
                  sourceDisplay="disclosure"
                  ref={props.figureRefs?.[kind]}
                />
              );
            })
          ) : (
            <p>Select a node to inspect its row and column.</p>
          )}
        </div>
      </div>
      <details>
        <summary className="cursor-pointer border-t border-border py-2 text-xs">
          All {nodes.length} {props.mode} nodes
        </summary>
        <div className="max-h-80 overflow-auto">
          <FinstackTable
            data={nodes}
            getRowId={(node) => node.key}
            caption={`${props.id} ${props.mode} nodes`}
            link={accepted}
            getRowKey={(node) => node.key}
            getCellKey={(node, column) =>
              column === "value" ? node.key : null
            }
            getActiveCell={(key) => ({ rowId: key, columnId: "value" })}
            columns={[
              {
                id: "expiry",
                accessorFn: (node) => node.expiry,
                header: labels.expiry,
                meta: { className: "text-right finstack-numeric" },
              },
              {
                id: "secondary",
                accessorFn: (node) => node.secondary,
                header: labels.secondary,
                meta: { className: "text-right finstack-numeric" },
              },
              {
                id: "value",
                accessorFn: (node) => node.value,
                header: labels.value,
                meta: { className: "text-right finstack-numeric" },
              },
            ]}
          />
        </div>
      </details>
    </section>
  );
}
