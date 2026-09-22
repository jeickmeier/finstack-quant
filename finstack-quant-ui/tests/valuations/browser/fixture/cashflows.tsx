import { createRoot } from "react-dom/client";
import { useState } from "react";
import { FinstackQueryProvider } from "./hooks/shared/use-finstack/use-finstack";
import { useCashflows } from "./hooks/valuations/use-cashflows/use-cashflows";
import { usePriceInstrument } from "./hooks/valuations/use-price-instrument/use-price-instrument";
import { CashflowViewer } from "./components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import fixture from "./fixture.json";
function App() {
  const [selected, setSelected] = useState(0);
  const [density, setDensity] = useState<"compact" | "comfortable">("compact");
  const { request } = fixture.cases[selected];
  const price = usePriceInstrument(request);
  const cashflows = useCashflows(request);
  return (
    <main className="mx-auto max-w-6xl space-y-3 p-4">
      <h1>Native cashflow export</h1>
      <nav aria-label="Fixture selection" className="flex gap-3">
        {fixture.cases.map((entry, index) => (
          <button key={entry.type} onClick={() => setSelected(index)}>
            {entry.type}
          </button>
        ))}
      </nav>
      <button
        onClick={() =>
          setDensity(density === "compact" ? "comfortable" : "compact")
        }
      >
        Toggle density
      </button>
      <p>
        Selected model:{" "}
        <output aria-label="Selected model">{request.model}</output>
      </p>
      {price.data && (
        <p>
          Valuation:{" "}
          <output aria-label="Valuation">
            {price.data.value.amount} {price.data.value.currency}
          </output>
        </p>
      )}
      {price.error && <p role="alert">{price.error.message}</p>}
      <CashflowViewer
        text={cashflows.data}
        loading={cashflows.isFetching || cashflows.workerStatus === "starting"}
        error={cashflows.error?.message}
        density={density}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <FinstackQueryProvider>
    <App />
  </FinstackQueryProvider>,
);
