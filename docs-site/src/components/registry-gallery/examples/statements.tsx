"use client";
import type { ExampleProps } from "./props";
import model from "../../../../../finstack-quant-ui/src/fixtures/statements/analyst-model.json";
import checkSuite from "../../../../../finstack-quant-ui/src/fixtures/statements/analyst-checks.json";
import rolledQ2 from "../../../../../finstack-quant-ui/src/fixtures/statements/roll-forward-2025Q2.json";
import rolledQ3 from "../../../../../finstack-quant-ui/src/fixtures/statements/roll-forward-2025Q3.json";
import rolledQ4 from "../../../../../finstack-quant-ui/src/fixtures/statements/roll-forward-2025Q4.json";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import { FinancialModelEditor } from "@/components/finstack/statements/components/financial-model-editor/financial-model-editor";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { StatementChart } from "@/components/finstack/statements/components/statement-chart/statement-chart";
import { StatementExplanation } from "@/components/finstack/statements/components/statement-diagnostics/statement-explanation";
import { StatementChecks } from "@/components/finstack/statements/components/statement-diagnostics/statement-checks";
import { StatementCheckReport } from "@/components/finstack/statements/components/statement-diagnostics/statement-check-report";
import { StatementsWorkbench } from "@/components/finstack/statements/blocks/statements-workbench/statements-workbench";
import { useEvaluateStatement } from "@/hooks/statements/use-evaluate-statement/use-evaluate-statement";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";

const modelJson = JSON.stringify(model);
const suiteJson = JSON.stringify(checkSuite);

export function EditorExample(_props: ExampleProps) {
  return <FinancialModelEditor defaultJson={modelJson} onSubmit={() => {}} />;
}

function ResultExample({
  kind,
}: {
  kind: "grid" | "chart" | "explanation" | "checks" | "report";
}) {
  const evaluated = useEvaluateStatement({ modelJson });
  const result = evaluated.data;
  const resultJson = result ? serializeHost(result) : null;
  const checkRequest = resultJson
    ? {
        modelJson,
        resultsJson: resultJson,
        configJson: suiteJson,
        kind: "suite" as const,
      }
    : null;
  const checks = useWorkerQuery((worker) => ({
    ...workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "gallery-statement-checks",
        checkRequest,
      ] as const,
      ready: checkRequest !== null,
      missing: "Statement worker is not ready",
      queryFn: (client) => client.call("runStatementChecks", checkRequest!),
    }),
    enabled:
      worker.status === "ready" && checkRequest !== null && kind === "report",
  }));
  if (evaluated.error) return <p role="alert">{evaluated.error.message}</p>;
  if (!result || !resultJson)
    return <p role="status">Evaluating example model…</p>;
  if (kind === "grid")
    return (
      <StatementGrid model={model as FinancialModelSpecWire} result={result} />
    );
  if (kind === "chart")
    return (
      <StatementChart
        model={model as FinancialModelSpecWire}
        result={result}
        nodeId="revenue"
      />
    );
  if (kind === "explanation")
    return (
      <StatementExplanation
        request={{
          modelJson,
          resultsJson: resultJson,
          nodeId: "revenue",
          period: "2025Q2",
        }}
      />
    );
  if (kind === "checks")
    return (
      <StatementChecks
        modelJson={modelJson}
        resultsJson={resultJson}
        defaultConfigJson={suiteJson}
      />
    );
  if (checks.error) return <p role="alert">{checks.error.message}</p>;
  return checks.data ? (
    <StatementCheckReport report={checks.data} />
  ) : (
    <p role="status">Running native check…</p>
  );
}
export function GridExample(_props: ExampleProps) {
  return <ResultExample kind="grid" />;
}
export function ChartExample(_props: ExampleProps) {
  return <ResultExample kind="chart" />;
}
export function ExplanationExample(_props: ExampleProps) {
  return <ResultExample kind="explanation" />;
}
export function ChecksExample(_props: ExampleProps) {
  return <ResultExample kind="checks" />;
}
export function CheckReportExample(_props: ExampleProps) {
  return <ResultExample kind="report" />;
}
export function WorkbenchExample(_props: ExampleProps) {
  // Illustrative service responses; each complete model was evaluated and validated by WASM.
  const updates: Record<string, unknown> = {
    "2025Q2": rolledQ2,
    "2025Q3": rolledQ3,
    "2025Q4": rolledQ4,
  };
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
