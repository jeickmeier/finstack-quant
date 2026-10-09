"use client";
import type { ExampleProps } from "./props";
import { StatementExplanation } from "@/components/finstack/statements/components/statement-diagnostics/statement-explanation";
import data from "../data.json";

const request = {
  modelJson: JSON.stringify(data.statementModel),
  resultsJson: JSON.stringify(data.statementResult),
  nodeId: "revenue",
  period: "2025Q2",
};

export function Example(_props: ExampleProps) {
  return <StatementExplanation request={request} />;
}
