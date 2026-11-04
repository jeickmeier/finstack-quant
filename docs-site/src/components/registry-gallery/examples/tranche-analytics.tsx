"use client";
import type { ExampleProps } from "./props";
import { TrancheAnalytics } from "@/components/finstack/valuations/components/tranche-analytics/tranche-analytics";
import type {
  MetricFieldUnit,
  TrancheSolveFieldUnits,
  OasResult,
  TrancheMetrics,
} from "finstack-quant-wasm";
import { trancheAnalytics } from "../fixtures";

// Captured native valuations.instruments results for the floating SENIOR note
// of the CLO mezzanine regression golden at a 25% clean price; this item needs
// no worker.
const floating = trancheAnalytics.cases.find(
  (entry) => entry.name === "floating",
)!;
const results = floating.results;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <TrancheAnalytics
      trancheId={floating.trancheId}
      price={String(floating.marketPricePct)}
      metrics={{ value: results.metrics.value as TrancheMetrics }}
      metricUnits={{ value: results.metricUnits.value as MetricFieldUnit[] }}
      solveUnits={{ value: results.solveUnits.value as TrancheSolveFieldUnits }}
      breakevenCdr={{ value: results.breakevenCdr.value as number }}
      discountMargin={{ value: results.discountMargin.value as number }}
      oas={{ value: results.oas.value as OasResult }}
      // Shows the OasConfig disclosure; the captured OAS used Rust's default,
      // so this static example discards an applied configuration.
      onOasConfigChange={() => {}}
      schedule={{
        value: results.schedule.value,
      }}
      density={density}
    />
  );
}
