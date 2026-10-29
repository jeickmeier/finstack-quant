"use client";
import type { MarginCall, VmResult } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { MarginCallTable } from "@/components/finstack/margin/components/margin-call-table/margin-call-table";
import data from "../data.json";
// Native variation margin under the Rust USD regulatory CSA on 2024-06-17:
// a USD 1,000,000 collect, the schedule of four exposures and the call dates
// from 2024-06-14 to 2024-06-28; this item needs no worker.
const collect = data.vm.vm.collect.report;
const vm = { ...collect, result: collect.result as unknown as VmResult };
const calls = data.vm.calls.result as unknown as MarginCall[];
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MarginCallTable
      vm={vm}
      calls={calls}
      callDates={data.vm.callDates.result}
      density={density}
    />
  );
}
