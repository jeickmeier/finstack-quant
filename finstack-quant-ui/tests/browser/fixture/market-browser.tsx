import { useState } from "react";
import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import { useMarketValidator } from "./hooks/use-market-validator/use-market-validator";
import {
  MarketContextBrowser,
  marketTree,
  flattenMarket,
  marketPointer,
} from "./components/finstack/components/market-context-browser/market-context-browser";
import { MarketContextForm } from "./components/finstack/components/market-context-form/market-context-form";
import fixture from "./fixture.json";
function App() {
  const [market, setMarket] = useState(fixture.supplemental),
    validate = useMarketValidator();
  const negativeIndex = market.curves.findIndex((c) => c.id === "NEG-RATES");
  Object.assign(window, {
    marketProbe: {
      market,
      negativeIndex,
      knotIndex: market.curves
        .slice(0, negativeIndex)
        .filter((c) => "knot_points" in c && c.knot_points.length > 1).length,
      negativePointer: `/curves/${negativeIndex}/knot_points/1/1`,
      leaves: flattenMarket(marketTree(market))
        .filter((e) => !e.children.length)
        .map((e) => ({
          pointer: marketPointer(e.path),
          json: JSON.stringify(e.value),
        })),
    },
  });
  return (
    <main className="p-4 space-y-4">
      <h1>Full stored market browser</h1>
      <MarketContextBrowser
        state={market}
        surfaceOptions={() => ({ colorDomain: [0, 2] })}
      />
      <MarketContextForm
        defaultJson={JSON.stringify(fixture.supplemental)}
        validate={validate}
        onSubmit={(json) => setMarket(JSON.parse(json))}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <FinstackQueryProvider>
    <App />
  </FinstackQueryProvider>,
);
