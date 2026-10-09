"use client";
import type { ExampleProps } from "./props";
import { StatementChecks } from "@/components/finstack/statements/components/statement-diagnostics/statement-checks";
import data from "../data.json";

const modelJson = JSON.stringify(data.statementModel);
const resultsJson = JSON.stringify(data.statementResult);
const suiteJson = JSON.stringify(data.statementChecks);

export function Example(_props: ExampleProps) {
  return (
    <StatementChecks
      modelJson={modelJson}
      resultsJson={resultsJson}
      defaultConfigJson={suiteJson}
    />
  );
}
