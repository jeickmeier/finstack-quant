import { createRoot } from "react-dom/client";
import { useState } from "react";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import { useMarketValidator } from "./hooks/use-market-validator/use-market-validator";
import { usePriceInstrument } from "./hooks/use-price-instrument/use-price-instrument";
import { MarketContextForm } from "./components/finstack/components/market-context-form/market-context-form";
import { marketModule } from "./components/finstack/components/market-context-form/market";
import {
  CurveChart,
  type CurveState,
} from "./components/finstack/components/curve-chart/curve-chart";
import { JsonViewer } from "./components/finstack/primitives/json-viewer/json-viewer";
import { serializeHost } from "./lib/finstack/codec.mjs";
import fixture from "./fixture.json";
function App() {
  const [marketJson, setMarketJson] = useState(fixture.request.marketJson);
  const validate = useMarketValidator();
  const price = usePriceInstrument({ ...fixture.request, marketJson });
  const market = marketModule.codec.parse(marketJson) as {
    curves: CurveState[];
  };
  return (
    <main className="mx-auto max-w-6xl space-y-4 p-4">
      <h1 className="text-xl">Market editor</h1>
      <MarketContextForm
        defaultJson={fixture.request.marketJson}
        validate={validate}
        onSubmit={setMarketJson}
      />
      <CurveChart curves={market.curves} />
      <JsonViewer label="Applied market JSON" text={marketJson} />
      {price.data && (
        <JsonViewer
          label="Native valuation JSON"
          text={serializeHost(price.data)}
        />
      )}
      {price.error && <p role="alert">{price.error.message}</p>}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <FinstackQueryProvider>
    <App />
  </FinstackQueryProvider>,
);
