"use client";
import type { ExampleProps } from "./props";
import { MonteCarloDiagnostics } from "@/components/finstack/components/monte-carlo-diagnostics/monte-carlo-diagnostics";
import {
  valuationCodec,
  type ValuationResult,
  type MonteCarloValuationDetails,
} from "@/lib/finstack/host";
import data from "../data.json";
const mc = valuationCodec.parse(
  data.details.cases.at(-1)!.resultJson,
) as ValuationResult;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MonteCarloDiagnostics
      data={mc.details!.data as MonteCarloValuationDetails}
      currency={mc.value.currency}
      density={density}
    />
  );
}
