import { createRoot } from "react-dom/client";
import { useState } from "react";
import { MeasuresGrid } from "./components/finstack/components/measures-grid/measures-grid";
import fixture from "./fixture.json";
function App() {
  const [density, setDensity] = useState<"compact" | "comfortable">("compact");
  const comparison = {
    ...fixture.result,
    instrument_id: "SUPPLIED-EUR-CONTEXT",
    as_of: "2026-03-31",
    value: { amount: "987654.321", currency: "EUR" },
    measures: { ytm: 0.043 },
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
        groups={fixture.groups}
        density={density}
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
