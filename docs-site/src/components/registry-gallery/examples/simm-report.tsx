"use client";
import type { ExampleProps } from "./props";
import { SimmReport } from "@/components/finstack/margin/components/simm-report/simm-report";
import { simmResult } from "../fixtures";
// The native indicative SIMM result of the schema example on 2025-01-15
// (default version and MPOR); this item needs no worker.
const result = simmResult;
export function Example({ density = "compact" }: ExampleProps) {
  return <SimmReport result={result} density={density} />;
}
