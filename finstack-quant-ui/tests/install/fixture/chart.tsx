"use client";
import { useEffect, useMemo, useRef, useState } from "react";
import { defineChart, dot, lineY, crosshair } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { createChartCursor, cursorHost } from "@tanstack/charts/cursor";
import { tooltip } from "@tanstack/charts/tooltip";
import { whenSelected } from "@tanstack/charts/selection";
import {
  FinstackChart,
  chartSelection,
  type FigureHandle,
} from "@/components/finstack/primitives/finstack-chart/finstack-chart";
import { figureExample } from "./figure-data";
const points = [
  { id: "a", x: 1, y: 12 },
  { id: "b", x: 2, y: 18 },
  { id: "c", x: 3, y: 15 },
];
declare global {
  interface Window {
    installedFigure?: () => Promise<{ svg: string; png: number[] }>;
  }
}
export function InstalledItem() {
  const figure = useRef<FigureHandle>(null);
  const [accepted, setAccepted] = useState<string | null>(null);
  const [activations, setActivations] = useState(0);
  const [detail, setDetail] = useState<string | null>(null);
  const [cursor] = useState(() => createChartCursor<number, number>());
  const selection = useMemo(
    () =>
      chartSelection<(typeof points)[number], number, number>(
        { selectedKey: accepted, select: setAccepted },
        (point) => point.id,
      ),
    [accepted],
  );
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          lineY(points, { x: "x", y: "y", key: "id" }),
          dot(points, { x: "x", y: "y", key: "id", r: 5 }),
          whenSelected(
            dot(points, {
              x: "x",
              y: "y",
              key: "id",
              r: 9,
              fill: "none",
              stroke: "var(--primary)",
              strokeWidth: 2,
            }),
            selection,
          ),
          crosshair({ x: true, y: false }),
        ],
        scales: {
          x: {
            scale: scaleLinear().domain([0.5, 3.5]),
            axis: { label: "Supplied coordinate" },
          },
          y: {
            scale: scaleLinear().domain([10, 20]),
            axis: { label: "Supplied value" },
          },
        },
        selection,
        focus: "group-x",
        cursor: {
          use: cursorHost,
          controller: cursor,
          mode: "focus",
          match: "x",
          pin: true,
        },
        tooltip: { use: tooltip },
      }),
    [selection, cursor],
  );
  useEffect(() => {
    window.installedFigure = async () => {
      const options = {
        width: 900,
        height: 600,
        scale: 2,
        theme: "light" as const,
      };
      return {
        svg: await (await figure.current!.exportSvg(options)).text(),
        png: Array.from(
          new Uint8Array(
            await (await figure.current!.exportPng(options)).arrayBuffer(),
          ),
        ),
      };
    };
    return () => {
      delete window.installedFigure;
    };
  }, []);
  return (
    <>
      <FinstackChart {...figureExample} ref={figure} width={880} height={560} />
      <button onClick={() => setAccepted("b")}>
        Select second observation
      </button>
      <button onClick={() => setAccepted(null)}>Clear selection</button>
      <output aria-label="Accepted selection">{accepted ?? "none"}</output>
      <output aria-label="Activations">{activations}</output>
      <output aria-label="Detail">{detail ?? "none"}</output>
      <FinstackChart
        definition={definition}
        ariaLabel="Independent interactive observations"
        title="Original observations"
        width={880}
        height={340}
        onSelect={() => setActivations((n) => n + 1)}
        renderTooltipBody={({ defaultBody, primaryPoint, pinned, dismiss }) => (
          <>
            {defaultBody}
            {pinned && primaryPoint && (
              <button
                onClick={() => {
                  setDetail(primaryPoint.datum.id);
                  dismiss();
                }}
              >
                Open original detail
              </button>
            )}
          </>
        )}
      />
      <FinstackChart
        definition={definition}
        ariaLabel="Independent linked comparison"
        width={880}
        height={300}
      />
    </>
  );
}
