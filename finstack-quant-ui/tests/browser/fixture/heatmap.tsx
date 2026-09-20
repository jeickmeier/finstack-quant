import { useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  FinstackChart,
  heatmap,
  type FigureHandle,
} from "./components/finstack/primitives/finstack-chart/finstack-chart";
const small = Array.from({ length: 9 }, (_, i) => ({
  key: `cell-${i}`,
  x: i % 3,
  y: Math.floor(i / 3),
  value: i - 4,
}));
const large = Array.from({ length: 169 }, (_, i) => ({
  key: `large-${i}`,
  x: i % 13,
  y: Math.floor(i / 13),
  value: i,
}));
function App() {
  const [selectedKey, select] = useState<string | null>(null);
  const ref = useRef<FigureHandle>(null),
    largeRef = useRef<FigureHandle>(null);
  const mapping = {
    x: (d: (typeof small)[number]) => d.x,
    y: (d: (typeof small)[number]) => d.y,
    value: (d: (typeof small)[number]) => d.value,
    key: (d: (typeof small)[number]) => d.key,
    xLabel: "Supplied x",
    yLabel: "Supplied y",
    valueLabel: "Raw value",
  };
  const preset = useMemo(
    () =>
      heatmap({
        ...mapping,
        data: small,
        palette: "diverging",
        domain: [-4, 0, 4],
        link: { selectedKey, select },
      }),
    [selectedKey],
  );
  const big = useMemo(
    () =>
      heatmap({
        ...mapping,
        data: large,
        palette: "sequential",
        domain: [0, 168],
      }),
    [],
  );
  Object.assign(window, {
    heatmapProbe: {
      export: async (which: string, theme: string) =>
        (
          await (which === "small" ? ref : largeRef).current!.exportSvg({
            width: 900,
            height: 650,
            theme: theme as "light" | "dark",
          })
        ).text(),
      original: small,
    },
  });
  return (
    <main>
      <h1>Caller supplied heatmaps</h1>
      <output aria-label="Accepted cell">{selectedKey ?? "none"}</output>
      <FinstackChart
        {...preset}
        ref={ref}
        width={900}
        height={650}
        ariaLabel="Small grid"
        title="Supplied heatmap"
        caption="Original values, no calculation"
        sources={[{ label: "Caller grid" }]}
        figureAnnotations={[{ text: "Supplied note", x: 0.6, y: 0.12 }]}
        onSelect={(p) => {
          if (p && !small.includes(p.datum)) throw new Error("Cloned datum");
        }}
        renderTooltipBody={({ points }) => (
          <span>Transient cell {points[0]?.datum.key}</span>
        )}
      />
      <FinstackChart
        {...big}
        ref={largeRef}
        width={900}
        height={650}
        ariaLabel="Large grid"
        title="13 by 13 supplied grid"
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
