"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import { StatementMonteCarlo } from "@/components/finstack/statements/components/statement-monte-carlo/statement-monte-carlo";
import data from "../data.json";
import { statementMonteCarlo } from "../fixtures";

// Captured native Evaluator.evaluateMonteCarlo of the analyst model with an
// illustrative log-normal revenue forecast, and the breach probability Rust
// reports for leverage above 2.25x on the same paths; this item needs no
// worker.
const results = statementMonteCarlo;
const { metric, threshold, probability } = data.statementMonteCarloBreach;

export function Example({ density = "compact" }: ExampleProps) {
  const [count, setCount] = useState(0);
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Illustrative stochastic revenue (log-normal growth). Simulations and
        breach tests submitted here:{" "}
        <output aria-label="Submission count">{count}</output>
      </p>
      <StatementMonteCarlo
        defaultConfig={data.statementMonteCarloConfig}
        onSubmit={() => setCount((n) => n + 1)}
        results={results}
        defaultBreach={{ metric, threshold }}
        onBreach={() => setCount((n) => n + 1)}
        breachProbability={probability}
        density={density}
      />
    </div>
  );
}
