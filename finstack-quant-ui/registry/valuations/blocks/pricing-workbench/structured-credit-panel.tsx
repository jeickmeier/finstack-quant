"use client";
import { useState } from "react";
import { useDealTranches } from "@/hooks/valuations/use-tranche-analytics/use-tranche-analytics";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { LazySection } from "./lazy-section";
import { ScenarioPanel } from "./scenario-panel";
import { scenarioGridModule } from "./scenario-grid";
import { PoolPanel } from "./pool-panel";
import { TranchePanel } from "./tranche-panel";
import { WaterfallPanel } from "./waterfall-panel";
import { CoveragePanel } from "./coverage-panel";
import { EnhancementPanel } from "./enhancement-panel";
import { EquityPanel } from "./equity-panel";
/**
 * Shows the completed deal's pool and waterfall reports, coverage tests, enhancement and equity tranche, selects one of its native tranches and
 * shows that tranche's analytics and scenarios. The host's seeds are optional: with no seed the selection is the first
 * returned tranche, a seed Rust does not know shows Rust's error, and the scenario grid falls back to the schema's Rust example.
 */
export function StructuredCreditPanel({
  request,
  seeds,
  scenariosOpen,
  onScenariosOpenChange,
  density,
}: {
  /** The completed request's deal, market and date. Remount for another deal. */
  request: { instrumentJson: string; marketJson: string; asOf: string };
  seeds?: {
    trancheId?: string;
    gridJson?: string;
    priceDomain?: readonly [number, number];
    trancheMetrics?: readonly string[];
  };
  scenariosOpen: boolean;
  onScenariosOpenChange(open: boolean): void;
  density?: "compact" | "comfortable";
}) {
  const [chosen, setChosen] = useState(seeds?.trancheId);
  const tranches = useDealTranches(request.instrumentJson);
  if (tranches.error)
    return (
      <p role="alert" className="text-sm text-error">
        {tranches.error.message}
      </p>
    );
  const ids = tranches.data?.tranches.map((tranche) => tranche.id);
  // A seed Rust does not know goes to Rust, whose error is what the host needs to see.
  const trancheId = chosen ?? ids?.[0];
  if (!ids || trancheId === undefined)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {ids ? "This deal has no tranches." : "Loading tranches…"}
      </p>
    );
  const selected = { ...request, trancheId };
  return (
    <div className="flex flex-col gap-3">
      <LazySection title="Pool report">
        <PoolPanel
          request={{
            instrumentJson: request.instrumentJson,
            asOf: request.asOf,
          }}
          density={density}
        />
      </LazySection>
      <LazySection title="Waterfall report">
        <WaterfallPanel request={request} density={density} />
      </LazySection>
      <LazySection title="Coverage tests">
        <CoveragePanel request={request} density={density} />
      </LazySection>
      <LazySection title="Credit enhancement and excess spread">
        <EnhancementPanel request={request} density={density} />
      </LazySection>
      <LazySection title="Equity tranche">
        <EquityPanel request={request} density={density} />
      </LazySection>
      <div className="w-72">
        <EnumField
          label="Tranche"
          value={trancheId}
          onValueChange={setChosen}
          options={ids.map((id) => ({ value: id, label: id }))}
        />
      </div>
      <TranchePanel
        key={JSON.stringify(selected)}
        request={selected}
        metrics={seeds?.trancheMetrics}
        density={density}
      />
      <details
        onToggle={(event) => onScenariosOpenChange(event.currentTarget.open)}
      >
        <summary>Scenarios</summary>
        {scenariosOpen && (
          <ScenarioPanel
            key={trancheId}
            request={{
              ...selected,
              gridJson:
                seeds?.gridJson ?? JSON.stringify(scenarioGridModule.example),
            }}
            priceDomain={seeds?.priceDomain}
          />
        )}
      </details>
    </div>
  );
}
