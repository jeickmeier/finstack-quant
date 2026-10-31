"use client";
import { useState } from "react";
import { useDealTranches } from "@/hooks/valuations/use-tranche-analytics/use-tranche-analytics";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { ScenarioPanel } from "./scenario-panel";
import { scenarioGridModule } from "./scenario-grid";
import { TranchePanel } from "./tranche-panel";
/**
 * Selects one of the completed deal's native tranches and shows its analytics and
 * scenarios. The host's seeds are optional: the selection falls back to the first
 * returned tranche and the scenario grid to the schema's Rust example.
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
  const ids = tranches.data?.tranches.map((tranche) => tranche.id) ?? [];
  const trancheId =
    chosen !== undefined && ids.includes(chosen) ? chosen : ids[0];
  if (tranches.error)
    return (
      <p role="alert" className="text-sm text-error">
        {tranches.error.message}
      </p>
    );
  if (trancheId === undefined)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {tranches.data ? "This deal has no tranches." : "Loading tranches…"}
      </p>
    );
  const selected = { ...request, trancheId };
  return (
    <div className="flex flex-col gap-3">
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
        <summary>Scenario prices</summary>
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
