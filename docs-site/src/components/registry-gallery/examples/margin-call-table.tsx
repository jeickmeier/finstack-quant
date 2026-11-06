"use client";
import type { ExampleProps } from "./props";
import { MarginCallTable } from "@/components/finstack/margin/components/margin-call-table/margin-call-table";
import { vm } from "../fixtures";
// Native variation margin under the Rust USD regulatory CSA on 2024-06-17:
// a USD 1,000,000 collect, the schedule of four exposures and the call dates
// from 2024-06-14 to 2024-06-28; this item needs no worker.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MarginCallTable
      vm={vm.vm.collect.report}
      calls={vm.calls.result}
      callDates={vm.callDates.result}
      density={density}
    />
  );
}
