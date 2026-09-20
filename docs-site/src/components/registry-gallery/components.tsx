"use client";
import { useState } from "react";
import { LinkedFigureExample } from "@/components/finstack/components/figure-example/linked-figures";
import { FigureExample } from "@/components/finstack/components/figure-example/figure-example";
import { ValuationSummary } from "@/components/finstack/components/valuation-summary/valuation-summary";
import { MeasuresGrid } from "@/components/finstack/components/measures-grid/measures-grid";
import { CurveChart } from "@/components/finstack/components/curve-chart/curve-chart";
import {
  PricingParamsForm,
  type PricingParams,
} from "@/components/finstack/components/pricing-params-form/pricing-params-form";
import { CashflowViewer } from "@/components/finstack/components/cashflow-viewer/cashflow-viewer";
import { MonteCarloDiagnostics } from "@/components/finstack/components/monte-carlo-diagnostics/monte-carlo-diagnostics";
import { ExplanationTrace } from "@/components/finstack/components/explanation-trace/explanation-trace";
import { CovenantReport } from "@/components/finstack/components/covenant-report/covenant-report";
import { ValuationDetails } from "@/components/finstack/components/valuation-details/valuation-details";
import { VolSurfaceChart } from "@/components/finstack/components/vol-surface-chart/vol-surface-chart";
import { FxDeltaQuotes } from "@/components/finstack/components/fx-delta-quotes/fx-delta-quotes";
import { FxSurfaceChart } from "@/components/finstack/components/fx-surface-chart/fx-surface-chart";
import { VolCubeExplorer } from "@/components/finstack/components/vol-cube-explorer/vol-cube-explorer";
import { FxMatrixGrid } from "@/components/finstack/components/fx-matrix-grid/fx-matrix-grid";
import { MarketContextBrowser } from "@/components/finstack/components/market-context-browser/market-context-browser";
import { CalibrationReport } from "@/components/finstack/components/calibration-report/calibration-report";
import { CalibrationFitChart } from "@/components/finstack/components/calibration-fit-chart/calibration-fit-chart";
import { ScenarioHeatmap } from "@/components/finstack/components/scenario-heatmap/scenario-heatmap";
import { CurveLinkExample } from "@/components/finstack/components/curve-link-example/curve-link-example";
import { PricingFormsExample } from "@/examples/pricing-forms";
import { PricingWorkbench } from "@/components/finstack/blocks/pricing-workbench/pricing-workbench";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import {
  valuationCodec,
  type ValuationResult,
  type MonteCarloValuationDetails,
} from "@/lib/finstack/host";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import type { CalibrationWire } from "@/lib/finstack/generated/types/calibration";
import type {
  CalibrationReport as NativeReport,
  ScenarioTable,
} from "finstack-quant-wasm";
import { NativeForms } from "./native-forms";
import data from "./data.json";
const market = data.market.supplemental as unknown as MarketContextStateWire;
const cubes = data.cubes.market as unknown as MarketContextStateWire;
const calibrated = data.calibration.at(-1)!;
const mc = valuationCodec.parse(
  data.details.cases.at(-1)!.resultJson,
) as ValuationResult;
const cashflows = data.cashflows.cases.find(
  (entry) => entry.type === "xccy_swap",
)!;
const scenario = data.scenarios.cases[0]!.table as ScenarioTable;
const fx = {
  id: "EURUSD",
  expiries: [0.5, 1],
  atm_vols: [0.08, 0.09],
  rr_25d: [0.01, 0.012],
  bf_25d: [0.005, 0.006],
  rr_10d: null,
  bf_10d: null,
};
export const nativeItems = new Set([
  "schema-form",
  "instrument-form",
  "market-context-form",
  "calibration-form",
  "cashflow-viewer",
  "fx-surface-chart",
  "vol-cube-explorer",
  "pricing-workbench",
]);
export function ComponentDemo({
  name,
  density,
  width,
  variant,
  instrument,
}: {
  name: string;
  density: "compact" | "comfortable";
  width: number;
  variant: string;
  instrument?: string;
}) {
  const [params, setParams] = useState<PricingParams>(data.bond.request),
    [severity, setSeverity] = useState(scenario.cells[0]!.severity),
    link = useLinkedSelection();
  const report = calibrated.result.result.step_reports[
    "USD-OIS"
  ] as unknown as NativeReport;
  switch (name) {
    case "schema-form":
    case "instrument-form":
    case "market-context-form":
    case "calibration-form":
      return <NativeForms name={name} />;
    case "figure-example":
      return variant === "interactive" ? (
        <LinkedFigureExample />
      ) : (
        <FigureExample />
      );
    case "valuation-summary":
      return <ValuationSummary result={data.bond.result} density={density} />;
    case "measures-grid":
      return (
        <MeasuresGrid
          result={data.bond.result}
          groups={data.bond.groups}
          density={density}
        />
      );
    case "curve-chart":
      return <CurveChart curves={market.curves} width={width} height={420} />;
    case "pricing-params-form":
      return (
        <PricingParamsForm
          value={params}
          onValueChange={setParams}
          instrumentType="bond"
          models={{ bond: ["discounting", "rates_credit"] }}
          metrics={data.bond.groups}
        />
      );
    case "cashflow-viewer":
      return <CashflowViewer request={cashflows.request} density={density} />;
    case "monte-carlo-diagnostics":
      return (
        <MonteCarloDiagnostics
          data={mc.details!.data as MonteCarloValuationDetails}
          currency={mc.value.currency}
          density={density}
        />
      );
    case "explanation-trace":
      return (
        <>
          <p>
            Supplied canonical Rust trace example; pricing has no trace request
            input.
          </p>
          <ExplanationTrace
            value={{
              type: "calibration",
              entries: [
                {
                  kind: "calibration_iteration",
                  iteration: 0,
                  residual: 0.005,
                  knots_updated: ["2025-01-15"],
                  converged: false,
                },
              ],
            }}
            density={density}
          />
        </>
      );
    case "covenant-report":
      return (
        <CovenantReport
          value={JSON.parse(data.details.covenantReportsJson)}
          density={density}
        />
      );
    case "valuation-details":
      return <ValuationDetails result={mc} density={density} />;
    case "vol-surface-chart":
      return (
        <VolSurfaceChart
          surface={{
            id: "Stored observations",
            expiries: [0.5, 1, 2],
            strikes: [80, 100, 120],
            secondary_axis: "strike",
            quote_type: "black_lognormal",
            interpolation_mode: "total_variance",
            vols_row_major: [
              0.21, 0.2, 0.23, 0.22, 0.24, 0.26, 0.25, 0.27, 0.28,
            ],
          }}
          colorDomain={[0.15, 0.3]}
          width={width}
        />
      );
    case "fx-delta-quotes":
      return <FxDeltaQuotes surface={fx} />;
    case "fx-surface-chart":
      return (
        <FxSurfaceChart
          surface={fx}
          coordinates={[0.5, 1].flatMap((expiry) =>
            [1, 1.1, 1.2].map((strike) => ({ expiry, strike, forward: 1.12 })),
          )}
          colorDomain={[0, 1]}
          width={width}
          height={460}
        />
      );
    case "vol-cube-explorer":
      return (
        <VolCubeExplorer
          cube={cubes.vol_cubes.find((cube) =>
            variant === "shifted"
              ? cube.id === "SHIFTED-BLACK"
              : cube.id !== "SHIFTED-BLACK",
          )!}
          initialStrike={variant === "shifted" ? -0.005 : 0.05}
          initialConvention={
            variant === "shifted" ? "black_lognormal" : "normal"
          }
          colorDomains={{ normal: [0, 0.1], black_lognormal: [0, 2] }}
          width={width}
          height={460}
        />
      );
    case "fx-matrix-grid":
      return <FxMatrixGrid state={market.fx} />;
    case "market-context-browser":
      return <MarketContextBrowser state={market} />;
    case "calibration-report":
      return <CalibrationReport stepId="USD-OIS" report={report} link={link} />;
    case "calibration-fit-chart":
      return (
        <CalibrationFitChart
          stepId="USD-OIS"
          report={report}
          marketData={
            calibrated.input.market_data as CalibrationWire["market_data"]
          }
          link={link}
          width={width}
          height={460}
          caption="Returned residuals in native solver units"
          sources={[{ label: "Native calibration result" }]}
        />
      );
    case "scenario-heatmap":
      return (
        <ScenarioHeatmap
          table={scenario}
          severity={severity}
          onSeverityChange={setSeverity}
          priceDomain={[80, 140]}
          width={width}
          height={460}
          link={link}
        />
      );
    case "curve-link-example":
      return <CurveLinkExample />;
    case "pricing-forms-example":
      return <PricingFormsExample />;
    case "pricing-workbench":
      return (
        <PricingWorkbench
          defaultInstrumentType={instrument}
          defaultRequest={
            variant === "equity-option"
              ? {
                  ...data.cashflows.cases.find(
                    (entry) => entry.type === "equity_option",
                  )!.request,
                  metrics: ["delta", "gamma", "vega", "theta"],
                }
              : variant === "structured-credit"
                ? data.details.cases.find(
                    (entry) => entry.type === "structured_credit",
                  )?.request
                : data.bond.request
          }
          density={density}
          defaultCalibrationJson={JSON.stringify(data.calibration[0]!.input)}
        />
      );
    default:
      throw new Error(`Missing component harness: ${name}`);
  }
}
