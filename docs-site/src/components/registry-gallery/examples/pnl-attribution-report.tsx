"use client";
import type { ExampleProps } from "./props";
import { PnlAttributionReport } from "@/components/finstack/attribution/components/pnl-attribution-report/pnl-attribution-report";
import { pnlAttribution } from "../fixtures";
// A convertible's one-day credit widening under Rust's default waterfall
// order, with Rust's explain text, residual check and share of total.
const entry = pnlAttribution.cases.find((entry) => entry.name === "waterfall")!;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PnlAttributionReport
      attribution={entry.attribution}
      buckets={entry.buckets}
      explain={entry.explain}
      residualWithinTolerance={entry.residualWithinTolerance}
      density={density}
    />
  );
}
