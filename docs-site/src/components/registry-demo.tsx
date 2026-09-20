"use client";
import { PricingWorkbench } from "@/components/finstack/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "@/hooks/use-finstack/use-finstack";
export function RegistryDemo() {
  return (
    <div className="not-prose finstack-surface">
      <FinstackQueryProvider>
        <PricingWorkbench />
      </FinstackQueryProvider>
    </div>
  );
}
