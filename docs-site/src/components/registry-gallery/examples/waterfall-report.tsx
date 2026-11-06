"use client";
import type { ExampleProps } from "./props";
import { WaterfallReport } from "@/components/finstack/valuations/components/waterfall-report/waterfall-report";
import { waterfallReport } from "../fixtures";

// Captured native runSimulation of the example CLO cut to one year with an OC test
// it fails, so cash diverts each period; this item needs no worker.
const diverting = waterfallReport.cases.find(
  (entry) => entry.name === "diverting",
)!;

export function Example({ density = "compact" }: ExampleProps) {
  return <WaterfallReport run={diverting.run} density={density} />;
}
