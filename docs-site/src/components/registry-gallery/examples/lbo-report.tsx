"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { LboReport } from "@/components/finstack/statements/components/lbo-report/lbo-report";
import data from "../data.json";
import { statementLbo, statementLboRequest } from "../fixtures";

// Captured native statements_analytics.evaluateLbo of the analyst model
// under an illustrative dated transaction; this item needs no worker. Entry
// is the last actual quarter, and the exit bridge nets total_debt against cash.
const result = statementLbo;
const nodeIds = Object.keys(data.statementModel.nodes);

export function Example({ density = "compact" }: ExampleProps) {
  const [count, setCount] = useState(0);
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Illustrative dated transaction on quarterly EBITDA: Rust reads the
        entry earnings at the last actual quarter and the exit earnings at the
        projected exit quarter, and nets total_debt against cash at exit. The
        sponsor commitment equals the required cash, so the return is funded.
        Evaluations submitted here:{" "}
        <output aria-label="Submission count">{count}</output>
      </p>
      <LboReport
        defaultRequest={statementLboRequest}
        nodeIds={nodeIds}
        onSubmit={() => setCount((n) => n + 1)}
        result={result}
        density={density}
      />
    </div>
  );
}
