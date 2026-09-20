import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import { PricingWorkbench } from "./components/finstack/blocks/pricing-workbench/pricing-workbench";
createRoot(document.getElementById("root")!).render(
  <main className="mx-auto max-w-6xl p-4">
    <h1 className="mb-4 text-xl">Embedded bond pricing</h1>
    <FinstackQueryProvider>
      <PricingWorkbench />
    </FinstackQueryProvider>
  </main>,
);
