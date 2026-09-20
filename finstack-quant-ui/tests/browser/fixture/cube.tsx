import { useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import { useMarketValidator } from "./hooks/use-market-validator/use-market-validator";
import { VolCubeExplorer } from "./components/finstack/components/vol-cube-explorer/vol-cube-explorer";
import { MarketContextForm } from "./components/finstack/components/market-context-form/market-context-form";
import type { FigureHandle } from "./components/finstack/primitives/finstack-chart/finstack-chart";
import fixture from "./fixture.json";
function App() {
  const [market, setMarket] = useState(fixture.market),
    [shifted, setShifted] = useState(false),
    [outside, setOutside] = useState(false);
  const cube = market.vol_cubes.find((c) =>
    shifted ? c.id === "SHIFTED-BLACK" : c.id !== "SHIFTED-BLACK",
  )!;
  const ref = useRef<FigureHandle>(null),
    validate = useMarketValidator();
  Object.assign(window, {
    cubeProbe: {
      cube,
      export: async () =>
        (
          await ref.current!.exportSvg({
            width: 1000,
            height: 750,
            theme: "light",
          })
        ).text(),
    },
  });
  return (
    <main className="p-4 space-y-4">
      <h1>Checked cube evaluation</h1>
      <button onClick={() => setShifted(!shifted)}>Switch cube fixture</button>
      <button onClick={() => setOutside(!outside)}>
        Toggle outside coordinate
      </button>
      <MarketContextForm
        defaultJson={JSON.stringify(fixture.market)}
        validate={validate}
        onSubmit={(json) => setMarket(JSON.parse(json))}
      />
      <VolCubeExplorer
        cube={cube}
        initialStrike={0.05}
        initialConvention="normal"
        colorDomains={{ normal: [0, 0.1], black_lognormal: [0, 2] }}
        coordinates={outside ? [{ expiry: 999, tenor: 5 }] : undefined}
        width={900}
        height={750}
        figureRefs={{ heatmap: ref }}
        caption="Checked evaluator output"
        sources={[{ label: "Canonical cube state" }]}
        renderTooltipBody={({ points }) => (
          <span>Absolute strike {points[0]?.datum.coordinate.strike}</span>
        )}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <FinstackQueryProvider>
    <App />
  </FinstackQueryProvider>,
);
