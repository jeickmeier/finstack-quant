"use client";
import type { ExampleProps } from "./props";
import {
  DcfReport,
  type CorporateValuationResultWire,
} from "@/components/finstack/statements/components/dcf-report/dcf-report";
import type { DcfSensitivityResultWire } from "@/lib/finstack/generated/types/dcf_sensitivity_result";
import data from "../data.json";

// Captured native statements_analytics.evaluateDcf and dcfSensitivity of the
// analyst model under illustrative assumptions; this item needs no worker.
const result = data.statementDcf as unknown as CorporateValuationResultWire;
const sensitivity =
  data.statementDcfSensitivity as unknown as DcfSensitivityResultWire;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <DcfReport result={result} sensitivity={sensitivity} density={density} />
  );
}
