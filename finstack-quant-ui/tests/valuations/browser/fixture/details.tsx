import { useState } from "react";
import { createRoot } from "react-dom/client";
import { ValuationDetails } from "./components/finstack/valuations/components/valuation-details/valuation-details";
import { restore } from "./restore";
import fixture from "./fixture.json";
function App() {
  const [selected, setSelected] = useState(fixture.cases[0].type);
  const result = restore(fixture.cases.find((c) => c.type === selected)!);
  return (
    <main className="p-4 max-w-4xl mx-auto">
      <h1 className="text-xl">Valuation details</h1>
      <nav aria-label="Fixture selection" className="flex flex-wrap gap-3 my-3">
        {fixture.cases.map((c) => (
          <button key={c.type} onClick={() => setSelected(c.type)}>
            {c.type}
          </button>
        ))}
      </nav>
      <ValuationDetails result={result} />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
