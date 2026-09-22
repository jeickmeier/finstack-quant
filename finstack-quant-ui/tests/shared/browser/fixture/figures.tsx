import { figureExample } from "@/components/finstack/shared/chart/figure-example/figure-data";
import { useEffect, useRef } from "react";
import { readPresentation } from "./components/finstack/shared/chart/finstack-chart/presentation";
import { createRoot } from "react-dom/client";
import { defineChart, lineY, dot, text, ruleY } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { scalePoint } from "@tanstack/charts/scales/point";
import { scaleUtc } from "d3-scale";
import {
  FinstackChart,
  type FigureHandle,
} from "./components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FigureExample } from "./components/finstack/shared/chart/figure-example/figure-example";
const dates = [
  { date: new Date("2025-01-01T00:00Z"), value: -2e6 },
  { date: new Date("2025-02-01T00:00Z"), value: 1e6 },
  { date: new Date("2025-03-01T00:00Z"), value: 3e6 },
];
const dateFigure = {
  ariaLabel: "Dates and scientific values",
  title: "Dates and negative scientific ticks",
  caption: "Supplied timestamps stay UTC dates.",
  sources: [{ label: "Supplied date fixture" }],
  definition: defineChart({
    marks: [lineY(dates, { x: "date", y: "value", points: true })],
    scales: {
      x: {
        scale: scaleUtc,
        axis: {
          label: "Observation date (UTC)",
          ticks: {
            count: 3,
            format: (date: Date) => date.toISOString().slice(0, 10),
          },
        },
      },
      y: {
        scale: scaleLinear,
        axis: {
          label: "Supplied value",
          ticks: {
            count: 4,
            format: (value: number) => value.toExponential(1),
          },
        },
      },
    },
  }),
};
const categories = [
  { name: "Long category Alpha", value: -3 },
  { name: "Long category Beta", value: 1 },
  { name: "Long category Gamma", value: 4 },
];
const categoryFigure = {
  ariaLabel: "Categorical observations",
  title: "Category labels retain their supplied order",
  subtitle: "Labels may rotate explicitly; values remain unchanged",
  definition: defineChart({
    marks: [
      dot(categories, { x: "name", y: "value" }),
      ruleY([0]),
      text([categories[1]], {
        x: "name",
        y: "value",
        text: () => "Point label",
        dy: -12,
      }),
    ],
    scales: {
      x: {
        scale: scalePoint,
        axis: { label: "Supplied category", tickLabels: { rotate: -30 } },
      },
      y: {
        scale: scaleLinear,
        axis: { label: "Stored value", ticks: { count: 4 } },
      },
    },
  }),
};
function App() {
  const numeric = useRef<FigureHandle>(null),
    date = useRef<FigureHandle>(null),
    category = useRef<FigureHandle>(null);
  useEffect(() => {
    (window as any).figureProbe = {
      presentation(tokens: Record<string, string> = {}) {
        const host = document.createElement("div");
        host.style.fontSize = "20px";
        for (const [name, value] of Object.entries(tokens))
          host.style.setProperty(`--${name}`, value);
        document.body.append(host);
        try {
          const value = readPresentation(host);
          return {
            title: value.titleSize,
            body: value.bodySize,
            note: value.noteSize,
            cell: value.cellText.size,
            spacing: value.spacing,
          };
        } finally {
          host.remove();
        }
      },
      async export(name: string, format: "svg" | "png", options: any) {
        const handle = ({ numeric, date, category } as const)[name as "numeric"]
          .current!;
        const blob = await (format === "svg"
          ? handle.exportSvg(options)
          : handle.exportPng(options));
        if (format === "svg") return blob.text();
        return Array.from(new Uint8Array(await blob.arrayBuffer()));
      },
    };
  }, []);
  return (
    <main className="mx-auto max-w-5xl space-y-8 p-4">
      <header>
        <h1 className="text-xl font-medium">Publication figures</h1>
        <p className="text-sm text-muted-foreground">
          Supplied data · native scales and annotations · complete exports
        </p>
      </header>
      <FigureExample />
      <section data-case="numeric">
        <FinstackChart
          {...figureExample}
          title="An exceptionally long heading that must wrap without losing any supplied words in a narrow publication figure"
          ref={numeric}
          height={640}
        />
      </section>
      <section data-case="date">
        <FinstackChart {...dateFigure} ref={date} height={420} />
      </section>
      <section data-case="category">
        <FinstackChart {...categoryFigure} ref={category} height={420} />
      </section>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
