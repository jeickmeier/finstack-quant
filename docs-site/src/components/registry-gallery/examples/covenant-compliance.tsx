"use client";
import type { ExampleProps } from "./props";
import { CovenantCompliance } from "@/components/finstack/covenants/components/covenant-compliance/covenant-compliance";
import { covenantSeries } from "../fixtures";

// Captured native statements_analytics.evaluateCovenants over the analyst
// statement model (tests/covenants/generate.mjs); this item needs no worker.
const series = covenantSeries;

export function Example({ density = "compact" }: ExampleProps) {
  return <CovenantCompliance series={series} density={density} />;
}
