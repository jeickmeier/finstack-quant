"use client";
import data from "./registry-gallery/data.json";
import { Suspense } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
function WorkbenchRoute() {
  const search = useSearchParams();
  const type = search.get("instrument") ?? undefined;
  return (
    <div className="not-prose finstack-surface text-sm">
      <div className="mb-3 flex justify-end">
        <Link
          className="rounded-sm border border-border px-3 py-1.5 text-xs text-primary"
          href={`/registry-gallery/?${new URLSearchParams({ item: "pricing-workbench", focus: "1", ...(type ? { instrument: type } : {}) })}`}
        >
          Open full-screen workbench ↗
        </Link>
      </div>
      <FinstackQueryProvider>
        <PricingWorkbench
          defaultRequest={data.bond.request}
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
