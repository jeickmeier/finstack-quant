"use client";
import { useState } from "react";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { StatementChart } from "@/components/finstack/statements/components/statement-chart/statement-chart";
import { CreditAssessmentReport } from "@/components/finstack/statements/components/credit-assessment-report/credit-assessment-report";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { useCreditAssessment } from "@/hooks/statements/use-credit-assessment/use-credit-assessment";
import { usePlSummaryReport } from "@/hooks/statements/use-pl-summary-report/use-pl-summary-report";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { MetricEditor } from "./metric-editor";
import { NormalizationPanel } from "./normalization-panel";
import { CapitalStructurePanel } from "./capital-structure-panel";
import type { StatementSections, StatementView } from "./statements-panel";

/** Metrics tab: metric editing, ratio grid and trend, credit assessment, normalization and capital structure. */
export function RatiosPanel({
  model,
  modelJson,
  resultJson,
  nodeOptions,
  periodIds,
  sections,
  view,
  reported,
  rollupError,
  link,
  displayMode,
  activePeriod,
  currency,
  onCommit,
  density,
}: {
  model: FinancialModelSpecWire;
  /** Canonical model JSON of the evaluated result. */
  modelJson: string;
  /** Host-serialized reported StatementResult. */
  resultJson: string;
  nodeOptions: string[];
  /** The model's period ids, in order. */
  periodIds: string[];
  sections: StatementSections;
  /** Null while the annual rollup is pending or failed. */
  view: StatementView | null;
  reported: boolean;
  /** Native rollup error, shown as returned. */
  rollupError: string | null;
  link: LinkedSelection;
  displayMode: "exact" | "millions";
  activePeriod: string | null;
  currency: string | null;
  /** Validates and evaluates an edited model natively; resolves to the error message, or null once accepted. */
  onCommit(json: string): Promise<string | null>;
  density: "compact" | "comfortable";
}) {
  const [chartNode, setChartNode] = useState<string | null>(null);
  const chartId = sections.ratios.includes(chartNode ?? "")
    ? chartNode
    : sections.ratios[0];
  // Credit assessment and P&L summary read the reported result: the active period and the income lines over every reported period.
  const creditAssessment = useCreditAssessment(
    activePeriod ? { resultsJson: resultJson, period: activePeriod } : null,
  );
  const plSummary = usePlSummaryReport(
    sections.income.length
      ? {
          resultsJson: resultJson,
          lineItems: sections.income,
          periods: periodIds,
        }
      : null,
  );
  return (
    <div className="space-y-5">
      <MetricEditor model={model} onApply={onCommit} />
      <p className="text-sm text-muted-foreground">
        Every value below is calculated from the current financials by WASM.
      </p>
      {sections.ratios.length > 0 ? (
        <>
          {view ? (
            <StatementGrid
              title={
                reported
                  ? "Metrics, margins & covenant ratios"
                  : "Annual metrics, margins & covenant ratios"
              }
              model={view.model}
              result={view.result}
              link={link}
              nodeIds={sections.ratios}
              displayMode={displayMode}
              showType={false}
              showRaw={false}
              activePeriod={reported ? activePeriod : null}
              density={density}
            />
          ) : (
            <p className="text-sm text-muted-foreground" role="status">
              {rollupError ?? "Rolling up to annual periods…"}
            </p>
          )}
          <div className="rounded-lg border border-border bg-card p-4">
            <div className="max-w-md">
              <EnumField
                label="Trend"
                layout="select"
                value={chartId ?? ""}
                onValueChange={setChartNode}
                options={sections.ratios.map((id) => ({
                  value: id,
                  label: model.nodes[id]?.name ?? id,
                }))}
              />
            </div>
            {chartId && view && (
              <StatementChart
                model={view.model}
                result={view.result}
                nodeId={chartId}
                link={link}
              />
            )}
          </div>
        </>
      ) : (
        <p className="text-sm text-muted-foreground">
          No calculated metrics are supplied yet. Add one above.
        </p>
      )}
      <div className="rounded-lg border border-border bg-card p-4">
        <CreditAssessmentReport
          assessment={creditAssessment.data}
          loading={creditAssessment.isFetching}
          error={creditAssessment.error?.message}
          plSummary={
            sections.income.length ? (plSummary.data ?? null) : undefined
          }
          plSummaryLoading={plSummary.isFetching}
          plSummaryError={plSummary.error?.message}
          currency={currency}
          density={density}
        />
      </div>
      <div className="rounded-lg border border-border bg-card p-4">
        <NormalizationPanel
          resultJson={resultJson}
          nodeIds={nodeOptions}
          density={density}
        />
      </div>
      <div className="rounded-lg border border-border bg-card p-4">
        <CapitalStructurePanel
          resultJson={resultJson}
          modelJson={modelJson}
          nodeIds={nodeOptions}
          periodIds={periodIds}
          density={density}
        />
      </div>
    </div>
  );
}
