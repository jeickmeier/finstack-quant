"use client";
import { useMemo, useState } from "react";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  StatementMonteCarlo,
  type BreachRequest,
} from "@/components/finstack/statements/components/statement-monte-carlo/statement-monte-carlo";
import { useEvaluateMonteCarlo } from "@/hooks/statements/use-evaluate-monte-carlo/use-evaluate-monte-carlo";
import { useMonteCarloBreachProbability } from "@/hooks/statements/use-monte-carlo-breach-probability/use-monte-carlo-breach-probability";

/**
 * The workbench Monte Carlo tab: simulates the current model's stochastic
 * forecasts natively (statements.Evaluator.evaluateMonteCarlo) under the
 * submitted config, and tests a breach threshold on the returned paths
 * (statements.monteCarloBreachProbability). The config starts empty and the
 * simulation reruns natively when the model changes.
 */
export function MonteCarloPanel({
  modelJson,
  density,
}: {
  /** Canonical model JSON the workbench evaluated. */
  modelJson: string;
  density: "compact" | "comfortable";
}) {
  const [configJson, setConfigJson] = useState<string | null>(null);
  const simulation = useEvaluateMonteCarlo(
    configJson ? { modelJson, configJson } : null,
  );
  const resultsJson = useMemo(
    () => (simulation.data ? serializeHost(simulation.data) : null),
    [simulation.data],
  );
  const [breach, setBreach] = useState<BreachRequest | null>(null);
  const probability = useMonteCarloBreachProbability(
    breach && resultsJson ? { resultsJson, ...breach } : null,
  );
  return (
    <div className="max-w-5xl space-y-4">
      <div className="space-y-1">
        <h3 className="text-base font-medium">Monte Carlo</h3>
        <p className="text-sm text-muted-foreground">
          Simulates the model's stochastic forecasts over the forecast periods
          and reports the percentiles of every line natively. A breach
          probability needs the path table: simulate with include path data.
        </p>
      </div>
      <StatementMonteCarlo
        onSubmit={(json) => {
          setBreach(null);
          setConfigJson(json);
        }}
        results={configJson ? simulation.data : null}
        loading={simulation.isFetching}
        error={simulation.error?.message}
        onBreach={setBreach}
        breachProbability={breach && resultsJson ? probability.data : undefined}
        breachLoading={probability.isFetching}
        breachError={probability.error?.message}
        density={density}
      />
    </div>
  );
}
