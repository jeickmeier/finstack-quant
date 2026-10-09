"use client";
import type { ExampleProps } from "./props";
import { StatementsWorkbench } from "@/components/finstack/statements/blocks/statements-workbench/statements-workbench";
import data from "../data.json";

const modelJson = JSON.stringify(data.statementModel);
const suiteJson = JSON.stringify(data.statementChecks);
// Illustrative service responses; each complete model was evaluated and validated by WASM.
const updates: Record<string, unknown> = {
  "2025Q2": data.statementRollForward2025Q2,
  "2025Q3": data.statementRollForward2025Q3,
  "2025Q4": data.statementRollForward2025Q4,
};

export function Example(_props: ExampleProps) {
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">
        Fictional issuer and illustrative financials. Roll-forward responses
        use native forecast output as sample reported values.
      </p>
      <StatementsWorkbench
        defaultModelJson={modelJson}
        defaultCheckSuiteJson={suiteJson}
        onRollForward={async (_current, periodId) => {
          const update = updates[periodId];
          if (!update) throw new Error(`No illustrative actuals for ${periodId}`);
          return JSON.stringify(update);
        }}
      />
    </div>
  );
}
