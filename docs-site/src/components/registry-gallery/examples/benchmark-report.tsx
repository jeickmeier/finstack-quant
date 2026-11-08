"use client";
import type { ExampleProps } from "./props";
import { BenchmarkReport } from "@/components/finstack/analytics/components/benchmark-report/benchmark-report";
import { benchmarkReport } from "../fixtures";
// The native benchmark report of the tear-sheet panel with two illustrative
// already-excess factors, explicit daily frequency, cash and rolling window;
// this display item needs no worker.
const report = benchmarkReport;
// Host display choice: four decimal places of the returned decimal fractions.
const fourPlaces = (value: number) => value.toFixed(4);
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <BenchmarkReport
      report={report}
      formatValue={fourPlaces}
      density={density}
    />
  );
}
