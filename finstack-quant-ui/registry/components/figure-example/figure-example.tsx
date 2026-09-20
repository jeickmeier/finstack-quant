"use client";
import { Button } from "@/components/ui/button";
import { useRef, useState } from "react";
import {
  defineChart,
  lineY,
  dot,
  text,
  ruleX,
  ruleY,
  rect,
  arrow,
  colorLegend,
} from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import {
  FinstackChart,
  type FigureHandle,
} from "../../primitives/finstack-chart/finstack-chart";
/** Caller-supplied illustration values, never a pricing or market-data calculation. */
export const figureExample = {
  ariaLabel: "Supplied observations and explicit annotations",
  title: "Supplied observations with an explicit reference range",
  subtitle:
    "A complete figure: the same coordinates in a compact application and a publication export",
  caption:
    "Illustrative supplied values. Lines connect observations; they do not represent a priced interpolation.",
  sources: [
    { label: "Registry illustration", url: "https://example.com/source" },
    { label: "Annotation values supplied by the caller" },
  ],
  figureAnnotations: [{ text: "Figure note", x: 0.62, y: 0.68 }],
  definition: defineChart({
    marks: [
      rect([{ x1: 1.4, x2: 2.6, y1: -0.5, y2: 1.4 }], {
        x1: "x1",
        x2: "x2",
        y1: "y1",
        y2: "y2",
        fill: "var(--primary)",
        fillOpacity: 0.08,
        id: "range",
      }),
      ruleY([0], {
        stroke: "var(--par)",
        strokeDasharray: "4 3",
        id: "horizontal-reference",
      }),
      ruleX([2], {
        stroke: "var(--par)",
        strokeDasharray: "2 4",
        id: "vertical-reference",
      }),
      lineY(
        [
          { id: "a", x: 1, y: -0.4, series: "Supplied series" },
          { id: "b", x: 2, y: 0.7, series: "Supplied series" },
          { id: "c", x: 3, y: 1.2, series: "Supplied series" },
        ],
        { x: "x", y: "y", z: "series", key: "id", id: "observations" },
      ),
      dot([{ x: 2, y: 0.7 }], {
        x: "x",
        y: "y",
        fill: "var(--primary)",
        id: "point-label-anchor",
      }),
      arrow([{ x1: 1.5, y1: 1.2, x2: 2.05, y2: 0.73 }], {
        x1: "x1",
        y1: "y1",
        x2: "x2",
        y2: "y2",
        stroke: "var(--foreground)",
        id: "callout-arrow",
      }),
      text([{ x: 1.5, y: 1.23, label: "Supplied callout" }], {
        x: "x",
        y: "y",
        text: "label",
        dy: -12,
        id: "callout-label",
      }),
    ],
    scales: {
      x: {
        scale: scaleLinear().domain([0.7, 3.3]),
        axis: { label: "Supplied coordinate", ticks: { count: 5 } },
      },
      y: {
        scale: scaleLinear().domain([-0.6, 1.6]),
        axis: { label: "Stored value (raw)", ticks: { count: 4 } },
        grid: true,
      },
    },
    color: { legend: colorLegend() },
    pointer: false,
    keyboard: false,
  }),
};
/** Installable standalone example with an explicit 6×4-inch, 300-PPI publication export. */
export function FigureExample() {
  const figure = useRef<FigureHandle>(null);
  const [error, setError] = useState<string | null>(null);
  async function download(format: "svg" | "png") {
    try {
      const options = {
        width: 900,
        height: 600,
        scale: 2,
        theme: "light" as const,
      };
      const blob = await (format === "svg"
        ? figure.current!.exportSvg(options)
        : figure.current!.exportPng(options));
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `supplied-observations.${format}`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 0);
      setError(null);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    }
  }
  return (
    <section className="space-y-2 font-sans text-foreground">
      <div className="flex gap-2 print:hidden">
        <Button
          variant="outline"
          size="sm"

          onClick={() => void download("svg")}
        >
          Download SVG
        </Button>
        <Button
          variant="outline"
          size="sm"

          onClick={() => void download("png")}
        >
          Download PNG · 1800 × 1200
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
      <FinstackChart {...figureExample} ref={figure} height={520} />
    </section>
  );
}
