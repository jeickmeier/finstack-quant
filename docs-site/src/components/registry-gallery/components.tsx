"use client";
import type { ExampleProps } from "./examples/props";
import { Example as Example0 } from "./examples/schema-form";
import { Example as Example1 } from "./examples/instrument-form";
import { Example as Example2 } from "./examples/market-context-form";
import { Example as Example3 } from "./examples/calibration-form";
import { Example as Example4 } from "./examples/figure-example";
import { Example as Example5 } from "./examples/valuation-summary";
import { Example as Example6 } from "./examples/measures-grid";
import { Example as Example7 } from "./examples/curve-chart";
import { Example as Example8 } from "./examples/pricing-params-form";
import { Example as Example9 } from "./examples/cashflow-viewer";
import { Example as Example10 } from "./examples/monte-carlo-diagnostics";
import { Example as Example11 } from "./examples/explanation-trace";
import { Example as Example12 } from "./examples/covenant-report";
import { Example as Example13 } from "./examples/valuation-details";
import { Example as Example14 } from "./examples/vol-surface-chart";
import { Example as Example15 } from "./examples/fx-delta-quotes";
import { Example as Example16 } from "./examples/fx-surface-chart";
import { Example as Example17 } from "./examples/vol-cube-explorer";
import { Example as Example18 } from "./examples/fx-matrix-grid";
import { Example as Example19 } from "./examples/market-context-browser";
import { Example as Example20 } from "./examples/calibration-report";
import { Example as Example21 } from "./examples/calibration-fit-chart";
import { Example as Example22 } from "./examples/scenario-heatmap";
import { Example as Example23 } from "./examples/curve-link-example";
import { Example as Example24 } from "./examples/pricing-forms-example";
import { Example as Example25 } from "./examples/pricing-workbench";
import { Example as Example26 } from "./examples/dimensional-measures";
import { Example as Example27 } from "./examples/financial-model-editor";
import { Example as Example28 } from "./examples/statement-grid";
import { Example as Example29 } from "./examples/statement-chart";
import { Example as Example30 } from "./examples/statement-explanation";
import { Example as Example31 } from "./examples/statement-checks";
import { Example as Example32 } from "./examples/statement-check-report";
import { Example as Example33 } from "./examples/statements-workbench";
export const nativeItems = new Set([
  "schema-form",
  "instrument-form",
  "market-context-form",
  "calibration-form",
  "cashflow-viewer",
  "fx-surface-chart",
  "vol-cube-explorer",
  "pricing-workbench",
  "financial-model-editor",
  "statement-explanation",
  "statement-checks",
  "statements-workbench",
]);
const examples = {
  "schema-form": Example0,
  "instrument-form": Example1,
  "market-context-form": Example2,
  "calibration-form": Example3,
  "figure-example": Example4,
  "valuation-summary": Example5,
  "measures-grid": Example6,
  "curve-chart": Example7,
  "pricing-params-form": Example8,
  "cashflow-viewer": Example9,
  "monte-carlo-diagnostics": Example10,
  "explanation-trace": Example11,
  "covenant-report": Example12,
  "valuation-details": Example13,
  "vol-surface-chart": Example14,
  "fx-delta-quotes": Example15,
  "fx-surface-chart": Example16,
  "vol-cube-explorer": Example17,
  "fx-matrix-grid": Example18,
  "market-context-browser": Example19,
  "calibration-report": Example20,
  "calibration-fit-chart": Example21,
  "scenario-heatmap": Example22,
  "curve-link-example": Example23,
  "pricing-forms-example": Example24,
  "pricing-workbench": Example25,
  "dimensional-measures": Example26,
  "financial-model-editor": Example27,
  "statement-grid": Example28,
  "statement-chart": Example29,
  "statement-explanation": Example30,
  "statement-checks": Example31,
  "statement-check-report": Example32,
  "statements-workbench": Example33,
};
export function ComponentDemo({
  name,
  ...props
}: ExampleProps & { name: string }) {
  const Example = examples[name as keyof typeof examples];
  if (!Example) throw new Error(`Missing components example: ${name}`);
  return <Example {...props} />;
}
