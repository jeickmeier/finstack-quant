"use client";
import { useState } from "react";
import { CapitalStructureReport } from "@/components/finstack/statements/components/capital-structure-report/capital-structure-report";
import { useCapitalStructureSummary } from "@/hooks/statements/use-capital-structure-summary/use-capital-structure-summary";

interface CapitalStructurePanelProps {
  /** Host-serialized StatementResult the workbench evaluated, holding cs_cashflows. */
  resultJson: string;
  /** Canonical model JSON whose capital structure gives the instruments and payment classes. */
  modelJson: string;
  /** The model's node ids, offered for the EBITDA and cash nodes. */
  nodeIds: readonly string[];
  /** The model's period ids, offered for the reporting period. */
  periodIds: readonly string[];
  density: "compact" | "comfortable";
}
function CapitalStructureBody({
  resultJson,
  modelJson,
  nodeIds,
  periodIds,
  density,
}: CapitalStructurePanelProps) {
  const [configJson, setConfigJson] = useState<string | null>(null);
  const summary = useCapitalStructureSummary(
    configJson === null ? null : { resultJson, modelJson, configJson },
  );
  return (
    <CapitalStructureReport
      nodeIds={nodeIds}
      periodIds={periodIds}
      onSubmit={setConfigJson}
      summary={configJson === null ? null : summary.data}
      loading={summary.isFetching}
      error={configJson === null ? undefined : summary.error?.message}
      density={density}
    />
  );
}
/**
 * The debt stack of the evaluated model: a native CapitalStructureSummaryConfig
 * form and statements_analytics.capitalStructureSummary, in a disclosure whose
 * native calls run only once it is opened. A model with no capital structure
 * shows Rust's error.
 */
export function CapitalStructurePanel(props: CapitalStructurePanelProps) {
  const [open, setOpen] = useState(false);
  return (
    <details onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>Capital structure and debt stack</summary>
      {open && (
        <div className="mt-3">
          <CapitalStructureBody {...props} />
        </div>
      )}
    </details>
  );
}
