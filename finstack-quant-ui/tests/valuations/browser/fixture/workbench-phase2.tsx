import { createRoot } from "react-dom/client";
import { useState } from "react";
import { FinstackQueryProvider } from "./hooks/shared/use-finstack/use-finstack";
import { PricingWorkbench } from "./components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import cases from "./cases.json";
function App() {
  const [selected, setSelected] = useState(cases[0].id);
  return (
    <main className="mx-auto max-w-6xl p-4">
      <h1 className="text-xl">Instrument workbench</h1>
      <nav
        aria-label="Native fixture requests"
        className="flex flex-wrap gap-2"
      >
        {cases.map((entry) => (
          <button key={entry.id} onClick={() => setSelected(entry.id)}>
            {entry.id}
          </button>
        ))}
      </nav>
      <FinstackQueryProvider>
        <PricingWorkbench
          key={selected}
          defaultRequest={cases.find((entry) => entry.id === selected)!.request}
        />
      </FinstackQueryProvider>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
