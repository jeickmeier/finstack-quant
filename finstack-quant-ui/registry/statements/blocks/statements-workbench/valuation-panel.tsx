"use client";
import { useState } from "react";
import {
  DcfAssumptionsForm,
  type DcfAssumptionsSubmission,
} from "@/components/finstack/statements/components/dcf-assumptions-form/dcf-assumptions-form";
import { DcfReport } from "@/components/finstack/statements/components/dcf-report/dcf-report";
import { useEvaluateDcf } from "@/hooks/statements/use-evaluate-dcf/use-evaluate-dcf";
import { useDcfSensitivity } from "@/hooks/statements/use-dcf-sensitivity/use-dcf-sensitivity";

/**
 * The workbench Valuation tab: a native DCF of the current canonical model
 * and its enterprise-value tornado. Assumptions start empty (Rust supplies no
 * defaults for them) and the valuation reruns natively when the model changes.
 */
export function ValuationPanel({
  modelJson,
  nodeIds,
  marketJson,
  asOf,
  density,
}: {
  /** Canonical model JSON the workbench evaluated. */
  modelJson: string;
  /** Model node ids offered for node fields, in the workbench's order. */
  nodeIds: readonly string[];
  /** Host market and as-of date, passed to evaluateDcf as supplied. */
  marketJson?: string;
  asOf?: string;
  density: "compact" | "comfortable";
}) {
  const [assumptions, setAssumptions] =
    useState<DcfAssumptionsSubmission | null>(null);
  const dcf = useEvaluateDcf(
    assumptions ? { modelJson, ...assumptions, marketJson, asOf } : null,
  );
  const sensitivity = useDcfSensitivity(
    assumptions ? { modelJson, ...assumptions } : null,
  );
  return (
    <div className="max-w-5xl space-y-6">
      <section aria-label="Discounted cash flow" className="space-y-4">
        <div className="space-y-1">
          <h3 className="text-base font-medium">Discounted cash flow</h3>
          <p className="text-sm text-muted-foreground">
            Values the model's unlevered free cash flow natively
            (statements_analytics.evaluateDcf) and ranks the headline
            assumptions by enterprise-value impact (dcfSensitivity).
          </p>
        </div>
        <DcfAssumptionsForm
          nodeIds={nodeIds}
          onSubmit={setAssumptions}
          disabled={dcf.workerStatus !== "ready"}
          density={density}
        />
        {assumptions && (
          <DcfReport
            result={dcf.data}
            loading={dcf.isFetching}
            error={dcf.error?.message}
            sensitivity={sensitivity.data ?? null}
            sensitivityLoading={sensitivity.isFetching}
            sensitivityError={sensitivity.error?.message}
            sensitivityNote={
              marketJson !== undefined
                ? "dcfSensitivity takes no market, so it evaluates the model without the host market; its baseline can differ from the valuation above."
                : undefined
            }
            density={density}
          />
        )}
      </section>
    </div>
  );
}
