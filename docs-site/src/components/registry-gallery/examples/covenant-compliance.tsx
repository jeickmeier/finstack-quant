"use client";
import type { ExampleProps } from "./props";
import { CovenantCompliance } from "@/components/finstack/covenants/components/covenant-compliance/covenant-compliance";
import type { DatedCovenantReportsWire } from "@/lib/finstack/generated/types/dated_covenant_reports";
import data from "../data.json";

// Captured native CovenantEngine#evaluateSeries over the analyst statement
// model (tests/covenants/generate.mjs); this item needs no worker.
const series = data.covenantSeries as unknown as DatedCovenantReportsWire[];

export function Example({ density = "compact" }: ExampleProps) {
  return <CovenantCompliance series={series} density={density} />;
}
