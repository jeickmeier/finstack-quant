"use client";
import type { ExampleProps } from "./props";
import { PerformanceReport } from "@/components/finstack/analytics/components/performance-report/performance-report";
import { performanceReport } from "../fixtures";
// The native report of the tear-sheet panel with an April fiscal year, a
// 126-day window and an explicit rate and confidence; this item needs no worker.
const report = performanceReport;
// Host display choice: four decimal places of the returned decimal fractions.
const fourPlaces = (value: number) => value.toFixed(4);
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PerformanceReport
      report={report}
      formatValue={fourPlaces}
      density={density}
    />
  );
}
