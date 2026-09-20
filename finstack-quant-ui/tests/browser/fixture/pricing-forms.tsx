import { createRoot } from "react-dom/client";
import { PricingFormsExample } from "./examples/pricing-forms";
createRoot(document.getElementById("root")!).render(
  <main className="mx-auto max-w-5xl space-y-4 p-4">
    <h1 className="text-xl">Standalone instrument and pricing forms</h1>
    <PricingFormsExample />
  </main>,
);
