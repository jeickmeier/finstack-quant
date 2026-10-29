"use client";
import { useState } from "react";
import { CovenantEngineEditor } from "@/components/finstack/covenants/components/covenant-engine-editor/covenant-engine-editor";
import { CovenantCompliance } from "@/components/finstack/covenants/components/covenant-compliance/covenant-compliance";
import { useCovenantEngineValidator } from "@/hooks/covenants/use-covenant-engine-validator/use-covenant-engine-validator";
import { useCovenantTemplate } from "@/hooks/covenants/use-covenant-template/use-covenant-template";
import { useEvaluateCovenants } from "@/hooks/covenants/use-evaluate-covenants/use-evaluate-covenants";

/**
 * The workbench Covenants tab: edits a covenant engine (from the host's engine
 * or a Rust package template) and tests it natively against the evaluated
 * statements (statements_analytics.evaluateCovenants), one column per result
 * period. A metric the model lacks returns Rust's not_found error unchanged.
 */
export function CovenantsPanel({
  modelJson,
  resultsJson,
  nodeIds,
  defaultEngineJson,
  density,
}: {
  /** Canonical model JSON the workbench evaluated. */
  modelJson: string;
  /** Host-serialized StatementResult of that model. */
  resultsJson: string;
  /** The model's node ids, offered for every covenant metric id. */
  nodeIds: readonly string[];
  /** Host CovenantEngine JSON, evaluated on open; omitted starts empty. */
  defaultEngineJson?: string;
  density: "compact" | "comfortable";
}) {
  const validate = useCovenantEngineValidator();
  const loadTemplate = useCovenantTemplate();
  const [engineJson, setEngineJson] = useState<string | null>(
    defaultEngineJson ?? null,
  );
  const compliance = useEvaluateCovenants(
    engineJson ? { engineJson, modelJson, resultsJson } : null,
  );
  return (
    <div className="max-w-5xl space-y-6">
      <div className="space-y-1">
        <h3 className="text-base font-medium">Covenants</h3>
        <p className="text-sm text-muted-foreground">
          Every result period is tested natively on its last day with the
          covenants applicable on that date. Metric ids are statement node ids.
        </p>
      </div>
      <CovenantEngineEditor
        defaultJson={defaultEngineJson}
        validate={validate}
        loadTemplate={loadTemplate}
        onSubmit={setEngineJson}
        metricIds={nodeIds}
        density={density}
      />
      {engineJson === null ? (
        <p className="text-sm text-muted-foreground">
          Use covenants to test them against the statements.
        </p>
      ) : compliance.error ? (
        <p role="alert" className="text-error">
          {compliance.error.message}
        </p>
      ) : compliance.data ? (
        <CovenantCompliance series={compliance.data} density={density} />
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          Evaluating covenants…
        </p>
      )}
    </div>
  );
}
