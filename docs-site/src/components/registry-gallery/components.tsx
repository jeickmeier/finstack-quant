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
import { Example as Example34 } from "./examples/variance-table";
import { Example as Example35 } from "./examples/tranche-analytics";
import { Example as Example36 } from "./examples/vol-cube-slice";
import { Example as Example37 } from "./examples/market-scenario-builder";
import { Example as Example38 } from "./examples/market-scenario-report";
import { Example as Example39 } from "./examples/performance-report";
import { Example as Example40 } from "./examples/covenant-compliance";
import { Example as Example41 } from "./examples/portfolio-summary";
import { Example as Example42 } from "./examples/positions-table";
import { Example as Example43 } from "./examples/portfolio-metrics";
import { Example as Example44 } from "./examples/transition-matrix-heatmap";
import { Example as Example45 } from "./examples/simm-sensitivity-table";
import { Example as Example46 } from "./examples/simm-report";
import { Example as Example47 } from "./examples/simm-calculator";
import { Example as Example48 } from "./examples/pnl-attribution-report";
import { Example as Example49 } from "./examples/horizon-return-report";
import { Example as Example50 } from "./examples/scenario-pnl-table";
import { Example as Example51 } from "./examples/benchmark-report";
import { Example as Example52 } from "./examples/merton-calculator";
import { Example as Example53 } from "./examples/csa-editor";
import { Example as Example54 } from "./examples/margin-call-table";
import { Example as Example55 } from "./examples/portfolio-margin-report";
import { Example as Example56 } from "./examples/credit-scorecard";
import { Example as Example57 } from "./examples/portfolio-risk-report";
import { Example as Example58 } from "./examples/credit-assessment-report";
import { Example as Example59 } from "./examples/tornado-chart";
import { Example as Example60 } from "./examples/dcf-report";
import { Example as Example61 } from "./examples/dcf-assumptions-form";
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
  "variance-table": Example34,
  "tranche-analytics": Example35,
  "vol-cube-slice": Example36,
  "market-scenario-builder": Example37,
  "market-scenario-report": Example38,
  "performance-report": Example39,
  "covenant-compliance": Example40,
  "portfolio-summary": Example41,
  "positions-table": Example42,
  "portfolio-metrics": Example43,
  "transition-matrix-heatmap": Example44,
  "simm-sensitivity-table": Example45,
  "simm-report": Example46,
  "simm-calculator": Example47,
  "pnl-attribution-report": Example48,
  "horizon-return-report": Example49,
  "scenario-pnl-table": Example50,
  "benchmark-report": Example51,
  "merton-calculator": Example52,
  "csa-editor": Example53,
  "margin-call-table": Example54,
  "portfolio-margin-report": Example55,
  "credit-scorecard": Example56,
  "portfolio-risk-report": Example57,
  "credit-assessment-report": Example58,
  "tornado-chart": Example59,
  "dcf-report": Example60,
  "dcf-assumptions-form": Example61,
};
export function ComponentDemo({
  name,
  ...props
}: ExampleProps & { name: string }) {
  const Example = examples[name as keyof typeof examples];
  if (!Example) throw new Error(`Missing components example: ${name}`);
  return <Example {...props} />;
}
