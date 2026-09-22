import { useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { VolSurfaceChart } from "./components/finstack/core/components/vol-surface-chart/vol-surface-chart";
import { FxDeltaQuotes } from "./components/finstack/core/components/fx-delta-quotes/fx-delta-quotes";
import { surfaceNodes } from "./components/finstack/core/components/vol-surface-chart/stored";
import { useLinkedSelection } from "./hooks/shared/use-linked-selection/use-linked-selection";
import type { FigureHandle } from "./components/finstack/shared/chart/finstack-chart/finstack-chart";
import fixtures from "./fixture.json";
function App() {
  const [surface, setSurface] = useState(fixtures.stored);
  const link = useLinkedSelection();
  const heatmap = useRef<FigureHandle>(null),
    row = useRef<FigureHandle>(null),
    column = useRef<FigureHandle>(null);
  Object.assign(window, {
    surfaceProbe: {
      export: async (kind: string, theme: string) =>
        (
          await { heatmap, row, column }[kind]!.current!.exportSvg({
            width: 900,
            height: 700,
            theme: theme as "light" | "dark",
          })
        ).text(),
    },
  });
  return (
    <main className="p-4 space-y-4">
      <h1>Stored surface views</h1>
      <output aria-label="Accepted node">{link.selectedKey ?? "none"}</output>
      <button onClick={() => link.select(surfaceNodes(surface)[4]!.key)}>
        Select middle externally
      </button>
      <button
        onClick={() =>
          setSurface({
            ...surface,
            expiries: [0.5, 2],
            vols_row_major: [
              ...surface.vols_row_major.slice(0, 3),
              ...surface.vols_row_major.slice(6),
            ],
          })
        }
      >
        Remove selected expiry
      </button>
      <VolSurfaceChart
        surface={surface}
        link={link}
        colorDomain={[0, 0.3]}
        height={700}
        width={900}
        title="Canonical stored grid"
        caption="Stored coordinates only"
        sources={[{ label: "Native validated fixture" }]}
        figureAnnotations={[{ text: "Supplied annotation", x: 0.5, y: 0.12 }]}
        figureRefs={{ heatmap, row, column }}
        renderTooltipBody={({ points }) => (
          <span>Caller node {points[0]?.datum.value}</span>
        )}
      />
      <VolSurfaceChart
        surface={fixtures.normal}
        colorDomain={[0, 0.02]}
        width={900}
        height={600}
      />
      <VolSurfaceChart
        surface={fixtures.large}
        colorDomain={[0, 0.5]}
        width={900}
        height={700}
      />
      <FxDeltaQuotes surface={fixtures.fx} />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
