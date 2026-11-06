"use client";
import { useState } from "react";
import {
  DcfAssumptionsForm,
  type DcfAssumptionsSubmission,
} from "@/components/finstack/statements/components/dcf-assumptions-form/dcf-assumptions-form";
import { DcfReport } from "@/components/finstack/statements/components/dcf-report/dcf-report";
import { WaccCalculator } from "@/components/finstack/statements/components/wacc-calculator/wacc-calculator";
import type { WaccRequest } from "@/workers/finstack-contract";
import { useWacc } from "@/hooks/statements/use-wacc/use-wacc";
import { useEvaluateDcf } from "@/hooks/statements/use-evaluate-dcf/use-evaluate-dcf";
import { useDcfSensitivity } from "@/hooks/statements/use-dcf-sensitivity/use-dcf-sensitivity";
import { LboReport } from "@/components/finstack/statements/components/lbo-report/lbo-report";
import { useEvaluateLbo } from "@/hooks/statements/use-evaluate-lbo/use-evaluate-lbo";

/**
 * The workbench Valuation tab: a native DCF of the current canonical model
 * with its enterprise-value tornado, and a native LBO. Assumptions and the
 * LBO config start empty (Rust supplies no defaults for them) and both rerun
 * natively when the model changes. The WACC calculator sets the form's WACC
 * only on the user's click, and valuing stays a separate action.
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
  // Undefined until the calculator or an edit sets it, so the form holds the
  // WACC field until then (an empty string would hide a default WACC).
  const [waccText, setWaccText] = useState<string | undefined>(undefined);
  const [waccInputs, setWaccInputs] = useState<WaccRequest | null>(null);
  const calculated = useWacc(waccInputs);
  const [lboConfigJson, setLboConfigJson] = useState<string | null>(null);
  const lbo = useEvaluateLbo(
    lboConfigJson ? { modelJson, configJson: lboConfigJson } : null,
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
        <details className="rounded-md border border-border p-3">
          <summary className="cursor-pointer text-sm font-medium">
            WACC calculator
          </summary>
          <div className="pt-3">
            <WaccCalculator
              onSubmit={setWaccInputs}
              rate={calculated.data ?? null}
              loading={calculated.isFetching}
              error={calculated.error?.message}
              onApply={(rate) => setWaccText(String(rate))}
              disabled={calculated.workerStatus !== "ready"}
              density={density}
            />
          </div>
        </details>
        <DcfAssumptionsForm
          nodeIds={nodeIds}
          wacc={waccText}
          onWaccChange={setWaccText}
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
      <section aria-label="LBO" className="space-y-4">
        <div className="space-y-1">
          <h3 className="text-base font-medium">Leveraged buyout</h3>
          <p className="text-sm text-muted-foreground">
            Evaluates a sponsor buyout of the model natively
            (statements_analytics.evaluateLbo) from the transaction config.
          </p>
        </div>
        <LboReport
          nodeIds={nodeIds}
          onSubmit={setLboConfigJson}
          result={lboConfigJson ? lbo.data : null}
          loading={lbo.isFetching}
          error={lbo.error?.message}
          density={density}
        />
      </section>
    </div>
  );
}
