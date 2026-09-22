import { createRoot } from "react-dom/client";
import { useState } from "react";
import { MeasuresGrid } from "./components/finstack/components/measures-grid/measures-grid";
import type { MetricMetadata } from "finstack-quant-wasm";
import fixture from "./fixture.json";
function App() {
  const [density, setDensity] = useState<"compact" | "comfortable">("compact");
  const comparison = {
    ...fixture.result,
    instrument_id: "SUPPLIED-EUR-CONTEXT",
    as_of: "2026-03-31",
    value: { amount: "987654.321", currency: "EUR" },
    // Synthetic display cases; the primary valuation remains the native fixture.
    measures: {
      ytm: 0.043,
      "bucketed_cs01::SYNTHETIC-CREDIT::1y": -100,
      "bucketed_cs01::SYNTHETIC-CREDIT::3y": 50,
      "bucketed_cs01::SYNTHETIC-CREDIT::5y": 0,
      "bucketed_cs01::SYNTHETIC-OTHER::1y": 10,
      "bucketed_dv01::USD-OIS::30y": 2,
      "bucketed_dv01::USD-OIS::10y": -1,
    },
    meta: {
      numeric_mode: "f64",
      rounding: { mode: "bankers", output_scale_by_currency: { EUR: 2 } },
      fx_policy_applied: "supplied-context",
      version: "supplied-version",
    },
  };
  return (
    <main className="mx-auto max-w-6xl space-y-3 p-4">
      <h1>Supplied valuation results</h1>
      <p>Comparison bucket values are synthetic display examples.</p>
      <button
        onClick={() =>
          setDensity(density === "compact" ? "comfortable" : "compact")
        }
      >
        Toggle density
      </button>
      <MeasuresGrid
        result={fixture.result}
        compareTo={comparison}
        comparisonFormattedValue="EUR 987,654.32"
        metadata={fixture.metadata as MetricMetadata[]}
        comparisonMetadata={fixture.comparisonMetadata as MetricMetadata[]}
        density={density}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
