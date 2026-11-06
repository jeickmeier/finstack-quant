"use client";
import type { ExampleProps } from "./props";
import { HorizonReturnReport } from "@/components/finstack/scenarios/components/horizon-return-report/horizon-return-report";
import { horizonReturn } from "../fixtures";
// The example bond over a three-month roll plus a 50bp USD-OIS shift.
const entry = horizonReturn.cases.find(
  (entry) => entry.name === "roll-3m-50bp",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <HorizonReturnReport
      report={entry.report}
      attributionBuckets={entry.attributionBuckets}
      explain={entry.explain}
      density={density}
    />
  );
}
