"use client";
import type {
  EquityMetricsFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { EquityTrancheCard } from "@/components/finstack/valuations/components/equity-tranche-card/equity-tranche-card";
import data from "../data.json";

// Captured native equityMetrics at Rust's par default and equityMetricsFieldUnits of a
// three-class deal whose equity class is paid; this item needs no worker.
const paying = data.equityTranche.cases.find(
  (entry) => entry.name === "paying",
)!;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <EquityTrancheCard
      metrics={
        paying.result.value as unknown as ReturnType<
          StructuredCredit["equityMetrics"]
        >
      }
      units={data.equityTranche.units as EquityMetricsFieldUnits}
      purchasePrice=""
      density={density}
    />
  );
}
