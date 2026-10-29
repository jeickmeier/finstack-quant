"use client";
import type { ExampleProps } from "./props";
import type { CreditAssessmentWire } from "@/lib/finstack/generated/types/credit_assessment";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { CreditAssessmentReport } from "@/components/finstack/statements/components/credit-assessment-report/credit-assessment-report";
import data from "../data.json";

// Captured native statements_analytics.creditAssessment (2025Q4) and
// plSummaryReport of the analyst model; this item needs no worker.
const assessment =
  data.statementCreditAssessment as unknown as CreditAssessmentWire;
const plSummary = data.statementPlSummary as unknown as TableEnvelopeWire;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CreditAssessmentReport
      assessment={assessment}
      plSummary={plSummary}
      currency={data.statementModel.meta.currency}
      density={density}
    />
  );
}
