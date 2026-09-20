"use client";
import { Suspense } from "react";
import { useSearchParams } from "next/navigation";
import { PricingWorkbench } from "@/components/finstack/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "@/hooks/use-finstack/use-finstack";
function WorkbenchRoute() {
  const search = useSearchParams();
  const type = search.get("instrument") ?? undefined;
  return (
    <div className="not-prose finstack-surface">
      <FinstackQueryProvider>
        <PricingWorkbench
          key={type ?? "default"}
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
          defaultInstrumentType={type}
        />
      </FinstackQueryProvider>
    </div>
  );
}
export function RegistryDemo() {
  return (
    <Suspense fallback={<p role="status">Loading pricing workbench…</p>}>
      <WorkbenchRoute />
    </Suspense>
  );
}
