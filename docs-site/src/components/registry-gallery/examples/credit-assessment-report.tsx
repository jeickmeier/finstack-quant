"use client";
import type { ExampleProps } from "./props";
import { CreditAssessmentReport } from "@/components/finstack/statements/components/credit-assessment-report/credit-assessment-report";
import data from "../data.json";
import { statementCreditAssessment, statementPlSummary } from "../fixtures";

// Captured native statements_analytics.creditAssessment (2025Q4) and
// plSummaryReport of the analyst model; this item needs no worker.
const assessment = statementCreditAssessment;
const plSummary = statementPlSummary;

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
