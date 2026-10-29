"use client";
import type { ExampleProps } from "./props";
import { StatementsWorkbench } from "@/components/finstack/statements/blocks/statements-workbench/statements-workbench";
import data from "../data.json";

const modelJson = JSON.stringify(data.statementModel);
const suiteJson = JSON.stringify(data.statementChecks);
// Base, downside and severe scenarios; the Scenarios tab diffs each against its parent natively.
const scenarioSetJson = JSON.stringify(data.statementScenarios);
// The analyst covenant engine; the Covenants tab tests it natively on open.
const covenantEngineJson = JSON.stringify(data.covenantEngine);
// Illustrative service responses; each complete model was evaluated and validated by WASM.
const updates: Record<string, unknown> = {
  "2025Q2": data.statementRollForward2025Q2,
  "2025Q3": data.statementRollForward2025Q3,
  "2025Q4": data.statementRollForward2025Q4,
};

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Fictional issuer and illustrative financials. Roll-forward responses use
        native forecast output as sample reported values.
      </p>
      <StatementsWorkbench
        defaultModelJson={modelJson}
        defaultCheckSuiteJson={suiteJson}
        defaultScenarioSetJson={scenarioSetJson}
        defaultCovenantEngineJson={covenantEngineJson}
        density={density}
        onRollForward={async (_current, periodId) => {
          const update = updates[periodId];
          if (!update)
            throw new Error(`No illustrative actuals for ${periodId}`);
          return JSON.stringify(update);
        }}
      />
    </div>
  );
}
