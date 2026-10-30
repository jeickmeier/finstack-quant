"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import type { StatementResult } from "finstack-quant-wasm";
import type { StatementSensitivity as Sensitivity } from "@/workers/finstack-contract";
import {
  StatementSensitivity,
  type StatementSensitivityDefaults,
} from "@/components/finstack/statements/components/statement-sensitivity/statement-sensitivity";
import data from "../data.json";

// Captured native runSensitivity and generateTornadoEntries of the analyst
// model under an illustrative run; this item needs no worker.
const run = data.statementSensitivityRun as StatementSensitivityDefaults;
const captured = data.statementSensitivity as unknown as Sensitivity;
const result = data.statementResult as unknown as StatementResult;
const nodeIds = Object.keys(data.statementModel.nodes);
const periods = data.statementModel.periods.map((period) => period.id);

export function Example({ density = "compact" }: ExampleProps) {
  const [count, setCount] = useState(0);
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Illustrative ±10% moves of 2025Q4 revenue and operating expenses against
        EBITDA. Runs submitted here:{" "}
        <output aria-label="Submission count">{count}</output>
      </p>
      <StatementSensitivity
        result={result}
        nodeIds={nodeIds}
        periods={periods}
        defaultValues={run}
        onSubmit={() => setCount((n) => n + 1)}
        data={captured}
        density={density}
      />
    </div>
  );
}
