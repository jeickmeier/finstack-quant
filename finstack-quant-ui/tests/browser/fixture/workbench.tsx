import { createRoot } from "react-dom/client";
import { FinstackQueryProvider } from "./hooks/use-finstack/use-finstack";
import { PricingWorkbench } from "./components/finstack/blocks/pricing-workbench/pricing-workbench";
createRoot(document.getElementById("root")!).render(
  <main className="mx-auto max-w-6xl p-4">
    <h1 className="mb-4 text-xl">Embedded bond pricing</h1>
    <FinstackQueryProvider>
      <PricingWorkbench
        surfaceOptions={() => ({ colorDomain: [0, 1] })}
        cubeOptions={() => ({
          initialStrike: 0.05,
          initialConvention: "normal",
          colorDomains: { normal: [0, 0.1], black_lognormal: [0, 2] },
        })}
        fxOptions={() => ({
          coordinates: [{ expiry: 1, strike: 1.1, forward: 1.12 }],
          colorDomain: [0, 1],
        })}
        defaultInstrumentType={
          new URLSearchParams(location.search).get("instrument") ?? undefined
        }
      />
    </FinstackQueryProvider>
  </main>,
);
