"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { LboReport } from "@/components/finstack/statements/components/lbo-report/lbo-report";
import type { LboResultWire } from "@/lib/finstack/generated/types/lbo_result";
import data from "../data.json";

// Captured native statements_analytics.evaluateLbo of the analyst model under
// an illustrative transaction; this item needs no worker. The model has no
// net-debt line, so the config reads exit net debt from total_debt.
const result = data.statementLbo as unknown as LboResultWire;
const nodeIds = Object.keys(data.statementModel.nodes);

export function Example({ density = "compact" }: ExampleProps) {
  const [count, setCount] = useState(0);
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Illustrative transaction on quarterly EBITDA: Rust reads the entry
        metric at the model's first period and the exit metric at the exit
        period. Exit net debt is the gross total_debt line. Evaluations
        submitted here: <output aria-label="Submission count">{count}</output>
      </p>
      <LboReport
        defaultConfig={data.statementLboConfig}
        nodeIds={nodeIds}
        onSubmit={() => setCount((n) => n + 1)}
        result={result}
        density={density}
      />
    </div>
  );
}
