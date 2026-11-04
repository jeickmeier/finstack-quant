"use client";
import type { ExampleProps } from "./props";
import { VarianceTable } from "@/components/finstack/statements/components/variance-table/variance-table";
import data from "../data.json";
import { statementScenarioDiff } from "../fixtures";

// Captured native statements_analytics.scenarioDiff of the analyst downside
// scenario against its base parent; this item needs no worker.
const report = statementScenarioDiff[0]!.variance;
const nodes = data.statementModel.nodes as Record<string, { name?: string }>;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <VarianceTable
      report={report}
      caption="Downside against base, 2025 quarters"
      metricLabel={(metric) => nodes[metric]?.name ?? metric}
      density={density}
    />
  );
}
