"use client";
import type { ExampleProps } from "./props";
import { DcfReport } from "@/components/finstack/statements/components/dcf-report/dcf-report";
import { statementDcf, statementDcfSensitivity } from "../fixtures";

// Captured native statements_analytics.evaluateDcf and dcfSensitivity of the
// analyst model under illustrative assumptions; this item needs no worker.
const result = statementDcf;
const sensitivity = statementDcfSensitivity;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <DcfReport result={result} sensitivity={sensitivity} density={density} />
  );
}
