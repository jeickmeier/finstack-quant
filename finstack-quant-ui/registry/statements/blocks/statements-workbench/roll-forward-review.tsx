"use client";
import { useMemo, useState } from "react";
import type { StatementResult } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { VarianceTable } from "@/components/finstack/statements/components/variance-table/variance-table";
import { VarianceBridgeChart } from "@/components/finstack/statements/components/variance-bridge-chart/variance-bridge-chart";
import { ForecastBacktest } from "@/components/finstack/statements/components/forecast-backtest/forecast-backtest";
import { useRunVariance } from "@/hooks/statements/use-run-variance/use-run-variance";
import { useVarianceBridge } from "@/hooks/statements/use-variance-bridge/use-variance-bridge";
import { useBacktestForecast } from "@/hooks/statements/use-backtest-forecast/use-backtest-forecast";

/** A canonical model and the host-serialized StatementResult evaluated from it. */
export interface EvaluatedModel {
  modelJson: string;
  resultJson: string;
}
const baselineLabel = "Before roll forward";
const comparisonLabel = "After roll forward";
/** A returned cell as a number; a missing cell or non-finite sentinel is NaN, which Rust rejects. */
function cell(result: StatementResult, nodeId: string, period: string) {
  const value = result.nodes[nodeId]?.[period];
  return typeof value === "number" ? value : Number.NaN;
}
/** Toggle buttons over node ids; the selection keeps the offered order. */
function NodeToggles({
  label,
  ids,
  selected,
  onChange,
}: {
  label: string;
  ids: readonly string[];
  selected: readonly string[];
  onChange(next: string[]): void;
}) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap gap-1">
      {ids.map((id) => {
        const on = selected.includes(id);
        return (
          <Button
            key={id}
            type="button"
            variant="outline"
            size="sm"
            aria-pressed={on}
            onClick={() =>
              onChange(
                ids.filter((other) =>
                  other === id ? !on : selected.includes(other),
                ),
              )
            }
          >
            <span className="font-mono">{id}</span>
          </Button>
        );
      })}
    </div>
  );
}
/**
 * "Since last roll forward": compares the model the workbench held before
 * its last roll forward with the current one, natively. Only node ids present
 * in both results are offered; the compared periods are those forecast before
 * the roll and actual after it. The variance (runVariance), a bridge of one
 * metric across driver nodes (varianceBridge) and a backtest of the
 * pre-roll forecast against the reported values (backtestForecast) each run
 * once their nodes are chosen. Nothing is calculated here.
 */
export function RollForwardReview({
  previous,
  current,
  density,
}: {
  /** The model and result before the last roll forward. */
  previous: EvaluatedModel;
  /** The current model and result. */
  current: EvaluatedModel;
  density: "compact" | "comfortable";
}) {
  const { ids, periods } = useMemo(() => {
    const before = JSON.parse(previous.resultJson) as StatementResult;
    const after = JSON.parse(current.resultJson) as StatementResult;
    const priorModel = JSON.parse(previous.modelJson) as FinancialModelSpecWire;
    const model = JSON.parse(current.modelJson) as FinancialModelSpecWire;
    const forecastBefore = new Set(
      priorModel.periods
        .filter((period) => !period.is_actual)
        .map((period) => period.id),
    );
    return {
      ids: Object.keys(model.nodes).filter(
        (id) => id in before.nodes && id in after.nodes,
      ),
      periods: model.periods
        .filter((period) => period.is_actual && forecastBefore.has(period.id))
        .map((period) => period.id),
    };
  }, [
    previous.modelJson,
    previous.resultJson,
    current.modelJson,
    current.resultJson,
  ]);
  const [metrics, setMetrics] = useState<string[]>([]);
  const [target, setTarget] = useState("");
  const [drivers, setDrivers] = useState<string[]>([]);
  const [bridgePeriod, setBridgePeriod] = useState("");
  const [backtested, setBacktested] = useState("");
  const base = previous.resultJson;
  const comparison = current.resultJson;
  const variance = useRunVariance(
    metrics.length && periods.length
      ? {
          baseJson: base,
          comparisonJson: comparison,
          configJson: JSON.stringify({
            baseline_label: baselineLabel,
            comparison_label: comparisonLabel,
            metrics,
            periods,
          }),
        }
      : null,
  );
  const bridge = useVarianceBridge(
    target && bridgePeriod
      ? {
          baseJson: base,
          comparisonJson: comparison,
          targetMetric: target,
          period: bridgePeriod,
          drivers,
          baselineLabel,
          comparisonLabel,
        }
      : null,
  );
  const backtestRequest = useMemo(() => {
    if (!backtested || !periods.length) return null;
    const before = JSON.parse(base) as StatementResult;
    const after = JSON.parse(comparison) as StatementResult;
    return {
      actual: periods.map((period) => cell(after, backtested, period)),
      forecast: periods.map((period) => cell(before, backtested, period)),
    };
  }, [backtested, periods, base, comparison]);
  const backtest = useBacktestForecast(backtestRequest);
  const idOptions = ids.map((id) => ({ value: id, label: id }));
  return (
    <section
      aria-label="Since last roll forward"
      className="space-y-5 rounded-lg border border-border bg-card p-4"
    >
      <div className="space-y-1">
        <h3 className="font-semibold">Since last roll forward</h3>
        <p className="text-sm text-muted-foreground">
          {periods.length
            ? `Compares the model before the last roll forward with the current one over the periods it reported: ${periods.join(", ")}.`
            : "The last roll forward reported no period that was a forecast before it."}
        </p>
      </div>
      {periods.length > 0 && (
        <>
          <div className="space-y-3">
            <h4 className="text-sm font-medium">Variance</h4>
            <NodeToggles
              label="Variance metrics"
              ids={ids}
              selected={metrics}
              onChange={setMetrics}
            />
            {metrics.length > 0 ? (
              variance.data || variance.error || variance.isFetching ? (
                <VarianceTable
                  report={
                    variance.data ?? {
                      baseline_label: baselineLabel,
                      comparison_label: comparisonLabel,
                      rows: [],
                    }
                  }
                  caption="Variance since last roll forward"
                  loading={variance.isFetching}
                  error={variance.error?.message}
                  density={density}
                />
              ) : null
            ) : (
              <p className="text-sm text-muted-foreground">
                Choose the metrics to compare.
              </p>
            )}
          </div>
          <div className="space-y-3">
            <h4 className="text-sm font-medium">Variance bridge</h4>
            <div className="grid gap-3 md:grid-cols-2">
              <EnumField
                label="Bridge metric"
                layout="select"
                value={target}
                onValueChange={setTarget}
                options={idOptions}
              />
              <EnumField
                label="Bridge period"
                layout="select"
                value={bridgePeriod}
                onValueChange={setBridgePeriod}
                options={periods.map((id) => ({ value: id, label: id }))}
              />
            </div>
            <NodeToggles
              label="Bridge drivers"
              ids={ids}
              selected={drivers}
              onChange={setDrivers}
            />
            {bridge.error ? (
              <p role="alert" className="text-sm text-error">
                {bridge.error.message}
              </p>
            ) : bridge.data ? (
              <VarianceBridgeChart bridge={bridge.data} density={density} />
            ) : (
              <p role="status" className="text-sm text-muted-foreground">
                {bridge.isFetching
                  ? "Bridging the variance…"
                  : "Choose the bridge metric and period."}
              </p>
            )}
          </div>
          <div className="space-y-3">
            <h4 className="text-sm font-medium">Forecast backtest</h4>
            <div className="max-w-md">
              <EnumField
                label="Backtested metric"
                layout="select"
                value={backtested}
                onValueChange={setBacktested}
                options={idOptions}
              />
            </div>
            {backtested ? (
              <ForecastBacktest
                metrics={backtest.data}
                caption={`${labelFor(backtested)}: forecast before the roll forward against reported values, ${periods.join(", ")}`}
                loading={backtest.isFetching}
                error={backtest.error?.message}
                density={density}
              />
            ) : (
              <p className="text-sm text-muted-foreground">
                Choose the metric to backtest.
              </p>
            )}
          </div>
        </>
      )}
    </section>
  );
}
