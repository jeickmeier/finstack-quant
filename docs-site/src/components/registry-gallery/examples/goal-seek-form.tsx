"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { GoalSeekForm } from "@/components/finstack/statements/components/goal-seek-form/goal-seek-form";
import data from "../data.json";
import { statementGoalSeekRun, statementGoalSeek } from "../fixtures";

// Captured native goalSeek of the analyst model (solved without the updated
// model); this item needs no worker and has nothing to apply.
const run = statementGoalSeekRun;
const result = statementGoalSeek;
const nodeIds = Object.keys(data.statementModel.nodes);
const periods = data.statementModel.periods.map((period) => period.id);

export function Example({ density = "compact" }: ExampleProps) {
  const [count, setCount] = useState(0);
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Illustrative target: 2025Q4 EBITDA of 70m from 2025Q4 revenue, searched
        between 100m and 500m. Solves submitted here:{" "}
        <output aria-label="Submission count">{count}</output>
      </p>
      <GoalSeekForm
        nodeIds={nodeIds}
        periods={periods}
        defaultValues={run}
        onSubmit={() => setCount((n) => n + 1)}
        result={result}
        density={density}
      />
    </div>
  );
}
