"use client";
import { useMemo, type Ref } from "react";
import {
  defineChart,
  dot,
  colorLegend,
  type ChartAxisOptions,
  type ChartMark,
} from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { whenSelected } from "@tanstack/charts/selection";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import routes from "@/lib/finstack/generated/curve-views.json";
import { getCurveView } from "@/lib/finstack/views";
import {
  FinstackChart,
  chartSelection,
  type FigureHandle,
  type FigureInteractions,
} from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import type { FigureText } from "@/components/finstack/shared/chart/finstack-chart/presentation";
export type CurveState = MarketContextStateWire["curves"][number];
/** Original state and tuple references; no evaluation, conversion or interpolation. */
export interface CurvePoint {
  curve: CurveState;
  knot: readonly [number, number];
}
export interface CurveChartProps
  extends FigureText, FigureInteractions<CurvePoint, number, number> {
  /** Validated canonical entries. Same-variant entries overlay; variants get separate panels. */
  curves: readonly CurveState[];
  /** Distinguishes landmarks when multiple curve views share a page. */
  ariaLabel?: string;
  height?: number;
  width?: number;
  axes?: { x?: ChartAxisOptions<number>; y?: ChartAxisOptions<number> };
  /** Native annotation marks in stored coordinates, supplied separately for each variant. */
  annotations?: Partial<
    Record<CurveState["type"], readonly ChartMark<CurvePoint, number, number>[]>
  >;
  /** Caller-owned accepted state. A key mapping is mandatory when linking. */
  link?: {
    selectedKey: string | null;
    select(key: string | null): void;
    getPointKey(point: CurvePoint): string;
  };
  /** Each plotted variant exposes the shared SVG/PNG export handle. Field views have no figure. */
  figureRefs?: Partial<Record<CurveState["type"], Ref<FigureHandle>>>;
}
/** Group in input order and retain only coordinates declared by the generated schema. */
export function curvePanels(curves: readonly CurveState[]) {
  const groups = new Map<CurveState["type"], CurveState[]>();
  const ids = new Set<string>();
  for (const curve of curves) {
    const key = JSON.stringify([curve.type, curve.id]);
    if (ids.has(key))
      throw new TypeError(
        `Duplicate curve identifier: ${curve.type}/${curve.id}`,
      );
    ids.add(key);
    const group = groups.get(curve.type) ?? [];
    group.push(curve);
    groups.set(curve.type, group);
  }
  return [...groups].map(([type, states]) => {
    const route = routes.find((route) => route.type === type)!;
    const points: CurvePoint[] = [];
    for (const curve of states) {
      const view = getCurveView(curve);
      if (view.kind === "knots" && "knot_points" in curve)
        for (const knot of curve.knot_points) points.push({ curve, knot });
    }
    return { type, states, route, points };
  });
}
/** Point marks only; native axes, categorical legend, tooltip and optional accepted selection. */
export function curveDefinition(
  points: CurvePoint[],
  type: CurveState["type"],
  props: Pick<CurveChartProps, "axes" | "annotations" | "link">,
) {
  const selection = props.link
    ? chartSelection<CurvePoint, number, number>(
        props.link,
        props.link.getPointKey,
      )
    : undefined;
  const channels = {
    x: (point: CurvePoint) => point.knot[0],
    y: (point: CurvePoint) => point.knot[1],
    z: (point: CurvePoint) => point.curve.id,
    key: (point: CurvePoint) => JSON.stringify([point.curve.id, point.knot]),
    r: 5,
  };
  return defineChart({
    marks: [
      dot(points, { ...channels, id: "stored-knots" }),
      ...(selection
        ? [
            whenSelected(
              dot(points, {
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
      ...(props.annotations?.[type] ?? []),
    ],
    scales: {
      x: props.axes?.x ?? {
        scale: scaleLinear,
        axis: { label: "Stored coordinate" },
      },
      y: props.axes?.y ?? {
        scale: scaleLinear,
        axis: { label: "Stored value" },
      },
    },
    color: { legend: colorLegend() },
    selection,
    tooltip: {
      use: tooltip,
      content: (points) => ({
        rows: points.map((point) => ({
          label: `${point.datum.curve.id} · Stored coordinate ${String(point.datum.knot[0])}`,
          value: `Stored value ${String(point.datum.knot[1])}`,
        })),
      }),
    },
  });
}
function KnotPanel({
  panel,
  props,
}: {
  panel: ReturnType<typeof curvePanels>[number];
  props: CurveChartProps;
}) {
  const { axes, annotations, link } = props;
  const definition = useMemo(
    () =>
      curveDefinition(panel.points, panel.type, { axes, annotations, link }),
    [panel, axes, annotations, link],
  );
  const sources = useMemo(
    () => [
      {
        label: panel.route.source.description,
        url: panel.route.source.uri + panel.route.source.pointer,
      },
      ...(props.sources ?? []),
    ],
    [panel.route, props.sources],
  );
  return (
    <FinstackChart
      definition={definition}
      ariaLabel={`${props.ariaLabel ?? panel.type} stored curves`}
      title={props.title ?? panel.route.source.description}
      subtitle={props.subtitle}
      caption={props.caption}
      sources={sources}
      sourceDisplay="disclosure"
      figureAnnotations={props.figureAnnotations}
      width={props.width}
      height={props.height}
      ref={props.figureRefs?.[panel.type]}
      onSelect={props.onSelect}
      onFocusChange={props.onFocusChange}
      onFocusGroupChange={props.onFocusGroupChange}
      renderTooltipBody={props.renderTooltipBody}
    />
  );
}
/** Present supplied market state without importing or invoking a pricing engine. */
export function CurveChart(props: CurveChartProps) {
  const panels = useMemo(() => curvePanels(props.curves), [props.curves]);
  return (
    <div className="space-y-6 font-sans text-sm text-foreground">
      {panels.length === 0 && <p>No curves supplied</p>}
      {panels.map((panel) => (
        <section
          key={panel.type}
          aria-label={
            props.ariaLabel
              ? `${props.ariaLabel}: ${panel.type} panel`
              : `${panel.type} panel`
          }
        >
          {panel.route.route === "stored-knots" ? (
            panel.points.length ? (
              <KnotPanel panel={panel} props={props} />
            ) : (
              <p>{panel.route.source.description}: no stored knots supplied</p>
            )
          ) : (
            <>
              <h2 className="text-base font-semibold">
                {panel.route.source.description}
              </h2>
              {panel.states.map((curve) => {
                const view = getCurveView(curve);
                return (
                  view.kind === "fields" && (
                    <section
                      key={curve.id}
                      aria-label={curve.id}
                      className="mt-3 rounded-sm border border-border p-3"
                    >
                      <h3 className="font-medium">{curve.id}</h3>
                      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
                        {Object.entries(view.fields).map(([field, value]) => (
                          <div key={field} className="contents">
                            <dt>{field}</dt>
                            <dd className="overflow-auto whitespace-pre-wrap font-mono tabular-nums">
                              {typeof value === "string"
                                ? value
                                : JSON.stringify(value, null, 2)}
                            </dd>
                          </div>
                        ))}
                      </dl>
                    </section>
                  )
                );
              })}
            </>
          )}
        </section>
      ))}
    </div>
  );
}
