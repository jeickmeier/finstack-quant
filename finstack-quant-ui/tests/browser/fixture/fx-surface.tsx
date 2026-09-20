import { useState, useRef } from "react";
import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import { useMarketValidator } from "./hooks/use-market-validator/use-market-validator";
import { useLinkedSelection } from "./hooks/use-linked-selection/use-linked-selection";
import { FxSurfaceChart } from "./components/finstack/components/fx-surface-chart/fx-surface-chart";
import { MarketContextForm } from "./components/finstack/components/market-context-form/market-context-form";
import type { FigureHandle } from "./components/finstack/primitives/finstack-chart/finstack-chart";
import fixture from "./fixture.json";
function App() {
  const [market, setMarket] = useState(fixture.market);
  const [forward, setForward] = useState<number | undefined>();
  const validate = useMarketValidator(),
    link = useLinkedSelection(),
    ref = useRef<FigureHandle>(null);
  const surface = market.fx_delta_vol_surfaces[0];
  const coordinates = [0.5, 1].flatMap((expiry) =>
    [1, 1.1, 1.2].map((strike) => ({ expiry, strike, forward })),
  );
  Object.assign(window, {
    fxSurfaceProbe: {
      market,
      coordinates,
      export: async () =>
        (
          await ref.current!.exportSvg({
            width: 900,
            height: 700,
            theme: "light",
          })
        ).text(),
    },
  });
  return (
    <main className="p-4 space-y-4">
      <h1>Native FX volatility</h1>
      <label>
        Explicit forward
        <input
          aria-label="Explicit forward"
          type="number"
          value={forward ?? ""}
          onChange={(e) =>
            setForward(
              e.target.value === "" ? undefined : Number(e.target.value),
            )
          }
        />
      </label>
      <MarketContextForm
        defaultJson={JSON.stringify(fixture.market)}
        validate={validate}
        onSubmit={(json) => setMarket(JSON.parse(json))}
      />
      <output aria-label="Accepted sample">{link.selectedKey ?? "none"}</output>
      <FxSurfaceChart
        surface={surface}
        coordinates={coordinates}
        colorDomain={[0, 0.5]}
        link={link}
        width={900}
        height={700}
        caption="Native evaluated samples"
        sources={[{ label: "Explicit caller forwards" }]}
        figureRefs={{ heatmap: ref }}
        renderTooltipBody={({ points }) => (
          <span>Explicit forward {points[0]?.datum.coordinate.forward}</span>
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
