"use client";
import type { ComponentProps } from "react";
import type { StatementResult } from "finstack-quant-wasm";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { StatementExplanation } from "@/components/finstack/statements/components/statement-diagnostics/statement-explanation";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { ExportPanel } from "./export-panel";
import { RollupMethodsEditor, type RollupMethod } from "./rollup";

/** The columns and values the grids show: reported periods or the native annual rollup. */
export interface StatementView {
  model: FinancialModelSpecWire;
  result: StatementResult;
}
/** Line ids grouped by the statement their tags place them in, in model order. */
export interface StatementSections {
  income: string[];
  balance: string[];
  cash: string[];
  ratios: string[];
  other: string[];
}
/** Statements tab: the grids by statement, the annual rollup methods, export and the selected value's explanation. */
export function StatementsPanel({
  model,
  modelJson,
  resultJson,
  nodeOptions,
  sections,
  view,
  reported,
  methods,
  onMethodsChange,
  rollupError,
  link,
  displayMode,
  activeNode,
  activePeriod,
  onEditValue,
  density,
}: {
  model: FinancialModelSpecWire;
  /** Canonical model JSON of the evaluated result. */
  modelJson: string;
  /** Host-serialized reported StatementResult. */
  resultJson: string;
  nodeOptions: string[];
  sections: StatementSections;
  /** Null while the annual rollup is pending or failed. */
  view: StatementView | null;
  reported: boolean;
  methods: Record<string, RollupMethod> | null;
  onMethodsChange(methods: Record<string, RollupMethod>): void;
  /** Native rollup error, shown as returned. */
  rollupError: string | null;
  link: LinkedSelection;
  displayMode: "exact" | "millions";
  activeNode: string | null;
  activePeriod: string | null;
  onEditValue: ComponentProps<typeof StatementGrid>["onEditValue"];
  density: "compact" | "comfortable";
}) {
  return (
    <div className="space-y-6">
      <p className="text-xs text-muted-foreground">
        {reported
          ? "Select a period above to review it. Click a reported or forecast value to edit in exact units; calculated lines update from their drivers."
          : "Annual values are rolled up natively from the reported periods. Switch to Reported to edit values."}
      </p>
      {!reported && methods && (
        <RollupMethodsEditor
          model={model}
          nodeIds={nodeOptions}
          methods={methods}
          onChange={onMethodsChange}
        />
      )}
      {!reported && rollupError && (
        <p
          role="alert"
          className="rounded-md border border-error p-3 text-sm text-error"
        >
          {rollupError}
        </p>
      )}
      {!view && !rollupError && (
        <p className="text-sm text-muted-foreground" role="status">
          Rolling up to annual periods…
        </p>
      )}
      {view && (
        <div className="min-w-0 space-y-8">
          {(
            [
              ["Income statement", sections.income],
              ["Balance sheet & debt", sections.balance],
              ["Cash flow", sections.cash],
              ["Other lines", sections.other],
            ] as const
          ).map(
            ([title, ids]) =>
              ids.length > 0 && (
                <StatementGrid
                  key={title}
                  title={title}
                  model={view.model}
                  result={view.result}
                  link={link}
                  nodeIds={ids}
                  displayMode={displayMode}
                  showType={false}
                  showRaw={false}
                  activePeriod={reported ? activePeriod : null}
                  onEditValue={reported ? onEditValue : undefined}
                  density={density}
                />
              ),
          )}
        </div>
      )}
      <ExportPanel result={view?.result ?? null} density={density} />
      {reported && activeNode && activePeriod && (
        <details>
          <summary>Explain selected value</summary>
          <div className="mt-3">
            <StatementExplanation
              request={{
                modelJson,
                resultsJson: resultJson,
                nodeId: activeNode,
                period: activePeriod,
              }}
              density={density}
            />
          </div>
        </details>
      )}
    </div>
  );
}
