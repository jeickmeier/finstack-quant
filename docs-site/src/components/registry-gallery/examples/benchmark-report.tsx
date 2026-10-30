"use client";
import type { ExampleProps } from "./props";
import {
  BenchmarkReport,
  type BenchmarkReportData,
} from "@/components/finstack/analytics/components/benchmark-report/benchmark-report";
import data from "../data.json";
// The native benchmark report of the tear-sheet panel with two illustrative
// factors and every option left to Rust; this item needs no worker.
const report = data.benchmark.report as unknown as BenchmarkReportData;
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
