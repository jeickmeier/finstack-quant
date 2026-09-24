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
import {
  EditorExample,
  GridExample,
  ChartExample,
  ExplanationExample,
  ChecksExample,
  CheckReportExample,
  WorkbenchExample,
} from "./examples/statements";
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
  "statement-grid",
  "statement-chart",
  "statement-explanation",
  "statement-checks",
  "statement-check-report",
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
  "financial-model-editor": EditorExample,
  "statement-grid": GridExample,
  "statement-chart": ChartExample,
  "statement-explanation": ExplanationExample,
  "statement-checks": ChecksExample,
  "statement-check-report": CheckReportExample,
  "statements-workbench": WorkbenchExample,
};
export function ComponentDemo({
  name,
  ...props
}: ExampleProps & { name: string }) {
  const Example = examples[name as keyof typeof examples];
  if (!Example) throw new Error(`Missing components example: ${name}`);
  return <Example {...props} />;
}
