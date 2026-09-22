import { useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import {
  ScenarioHeatmap,
  ScenarioHeatmapPanel,
  scenarioKey,
} from "./components/finstack/valuations/components/scenario-heatmap/scenario-heatmap";
import { useLinkedSelection } from "./hooks/shared/use-linked-selection/use-linked-selection";
import type { FigureHandle } from "./components/finstack/shared/chart/finstack-chart/finstack-chart";
import fixture from "./fixture.json";
function App() {
  const [index, setIndex] = useState(0),
    [severity, setSeverity] = useState(fixture.grid.severities[0]),
    [example, setExample] = useState(false),
    ref = useRef<FigureHandle>(null),
    link = useLinkedSelection(),
    activated = useRef<string | null>(null);
  const table = fixture.cases[index].table;
  Object.assign(window, {
    scenarioProbe: {
      table,
      severity,
      selectedKey: link.selectedKey,
      get activated() {
        return activated.current;
      },
      export: async (theme: string) =>
        (
          await ref.current!.exportSvg({
            width: 1000,
            height: 700,
            theme: theme as "light" | "dark",
          })
        ).text(),
    },
  });
  return (
    <main className="p-4 space-y-4">
      <h1>Returned tranche scenario prices</h1>
      <button onClick={() => setIndex(index ? 0 : 1)}>
        Switch balance fixture
      </button>
      <button onClick={() => setExample(!example)}>
        Toggle standalone example
      </button>
      {example ? (
        <ScenarioHeatmapPanel table={table} priceDomain={[80, 140]} />
      ) : (
        <ScenarioHeatmap
          table={table}
          severity={severity}
          onSeverityChange={setSeverity}
          priceDomain={[80, 140]}
          width={950}
          height={700}
          link={link}
          figureRef={ref}
          caption="Original native prices"
          sources={[{ label: "Canonical Rust ScenarioCell.price" }]}
          figureAnnotations={[
            { text: "Native returned cells", x: 0.5, y: 0.06 },
          ]}
          onSelect={(point) => {
            if (point) {
              if (!table.cells.includes(point.datum))
                throw new Error("Scenario datum changed");
              activated.current = scenarioKey(table.tranche_id, point.datum);
            }
          }}
          renderTooltipBody={({ points }) => (
            <span>Transient price {points[0]?.datum.price}</span>
          )}
        />
      )}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
