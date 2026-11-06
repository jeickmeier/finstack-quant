"use client";
import { useState } from "react";
import type { StatementResult } from "finstack-quant-wasm";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  StatementSensitivity,
  type StatementSensitivitySubmission,
} from "@/components/finstack/statements/components/statement-sensitivity/statement-sensitivity";
import {
  GoalSeekForm,
  type GoalSeekSubmission,
} from "@/components/finstack/statements/components/goal-seek-form/goal-seek-form";
import { useRunSensitivity } from "@/hooks/statements/use-run-sensitivity/use-run-sensitivity";
import { useGoalSeek } from "@/hooks/statements/use-goal-seek/use-goal-seek";

/**
 * The workbench's sensitivity and goal-seek section. Both run only on an
 * explicit action against the model current at that moment; a later model
 * change hides the sensitivity until it is run again and blocks applying a
 * stale goal-seek solution. Apply hands the returned model to the block's
 * commit, which validates and re-evaluates it.
 */
export function SensitivityPanel({
  modelJson,
  result,
  nodeIds,
  periods,
  onApply,
  density,
}: {
  /** Canonical model JSON the workbench evaluated. */
  modelJson: string;
  /** Its evaluated result; sensitivity base values are read from it. */
  result: StatementResult;
  /** Model node ids, in the workbench's order. */
  nodeIds: readonly string[];
  /** Model period ids, in model order. */
  periods: readonly string[];
  /** The block's commit: validates and evaluates a model, returning an error message or null. */
  onApply(modelJson: string): Promise<string | null>;
  density: "compact" | "comfortable";
}) {
  const [sensitivity, setSensitivity] = useState<{
    modelJson: string;
    submission: StatementSensitivitySubmission;
  } | null>(null);
  const run = useRunSensitivity(
    sensitivity && sensitivity.modelJson === modelJson
      ? { modelJson, ...sensitivity.submission }
      : null,
  );
  const [solve, setSolve] = useState<{
    modelJson: string;
    submission: GoalSeekSubmission;
  } | null>(null);
  // Solved with the model written in, so Apply needs no second call.
  const seek = useGoalSeek(
    solve
      ? { modelJson: solve.modelJson, ...solve.submission, updateModel: true }
      : null,
  );
  return (
    <div className="max-w-5xl space-y-6">
      <section className="space-y-4">
        <div className="space-y-1">
          <h3 className="text-base font-medium">Sensitivity</h3>
          <p className="text-sm text-muted-foreground">
            Varies driver values by percentage moves of their evaluated values
            and ranks the target metric's response natively
            (statements_analytics.runSensitivity and generateTornadoEntries).
          </p>
        </div>
        <StatementSensitivity
          result={result}
          nodeIds={nodeIds}
          periods={periods}
          onSubmit={(submission) => setSensitivity({ modelJson, submission })}
          data={sensitivity?.modelJson === modelJson ? run.data : null}
          loading={run.isFetching}
          error={
            sensitivity?.modelJson === modelJson
              ? run.error?.message
              : undefined
          }
          disabled={run.workerStatus !== "ready"}
          density={density}
        />
      </section>
      <section className="space-y-4">
        <div className="space-y-1">
          <h3 className="text-base font-medium">Goal seek</h3>
          <p className="text-sm text-muted-foreground">
            Solves for the driver value that brings a target node to a target
            value natively (statements_analytics.goalSeek).
          </p>
        </div>
        <GoalSeekForm
          nodeIds={nodeIds}
          periods={periods}
          onSubmit={(submission) => setSolve({ modelJson, submission })}
          result={solve ? seek.data : null}
          loading={seek.isFetching}
          error={seek.error?.message}
          onApply={(model) => onApply(serializeHost(model))}
          applyBlocked={
            solve && solve.modelJson !== modelJson
              ? "The model changed after this solve; solve again before applying."
              : undefined
          }
          disabled={seek.workerStatus !== "ready"}
          density={density}
        />
      </section>
    </div>
  );
}
