import { createRoot } from "react-dom/client";
import { useState } from "react";
import { MeasuresGrid } from "./components/finstack/valuations/components/measures-grid/measures-grid";
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
      "bucketed_dv01::USD-OIS::15y": 0,
    },
    meta: {
      numeric_mode: "f64",
      rounding: { mode: "bankers", output_scale_by_currency: { EUR: 2 } },
      fx_policy_applied: "supplied-context",
      version: "supplied-version",
    },
  };
  return (
    <main className="mx-auto max-w-6xl space-y-4 p-4">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold">Valuation results</h1>
        <p className="text-sm text-muted-foreground">
          Supplied results with synthetic comparison bucket values.
        </p>
      </header>
      <div
        role="group"
        aria-label="Table density"
        className="flex items-center gap-1"
      >
        <span className="mr-2 text-xs font-medium text-muted-foreground">
          Density
        </span>
        {(["compact", "comfortable"] as const).map((option) => (
          <button
            key={option}
            type="button"
            aria-pressed={density === option}
            onClick={() => setDensity(option)}
            className="min-h-8 rounded-md border border-border px-3 py-1 text-xs font-medium hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring aria-pressed:bg-accent"
          >
            {option === "compact" ? "Compact" : "Comfortable"}
          </button>
        ))}
      </div>
      <MeasuresGrid
        result={fixture.result}
        compareTo={comparison}
        comparisonFormattedValue="EUR 987,654.32"
        metadata={fixture.metadata as MetricMetadata[]}
        comparisonMetadata={[
          ...(fixture.comparisonMetadata as MetricMetadata[]),
          ...(fixture.metadata as MetricMetadata[]).filter(
            (entry) => entry.key === "bucketed_dv01::USD-OIS::15y",
          ),
        ]}
        density={density}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
