"use client";
import type { ExampleProps } from "./props";
import { ValuationSummary } from "@/components/finstack/components/valuation-summary/valuation-summary";
import data from "../data.json";

export function Example({ density = "compact" }: ExampleProps) {
  return <ValuationSummary result={data.bond.result} density={density} />;
}
