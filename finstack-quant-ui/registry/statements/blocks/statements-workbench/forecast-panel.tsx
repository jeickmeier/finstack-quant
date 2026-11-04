"use client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { ForecastEditor } from "./forecast-editor";
import { RollForwardReview, type EvaluatedModel } from "./roll-forward-review";

/** Forecast tab: each driver's forecast method, adding a driver, and rolling forward to the next actual period. */
export function ForecastPanel({
  model,
  revision,
  onCommit,
  nextForecastId,
  onRollForward,
  rolling,
  ready,
  onLoadUpdated,
  previousRoll,
  current,
  density,
}: {
  model: FinancialModelSpecWire;
  /** Remounts the editors after each accepted model. */
  revision: number;
  /** Validates and evaluates an edited model natively; resolves to the error message, or null once accepted. */
  onCommit(json: string): Promise<string | null>;
  /** The first forecast period, rolled forward next; undefined when every period is actual. */
  nextForecastId: string | undefined;
  /** Requests the next complete model from the host; omitted hides the button. */
  onRollForward?: () => void;
  rolling: boolean;
  /** Whether the statement runtime is ready. */
  ready: boolean;
  /** Opens the tab that loads pasted financials. */
  onLoadUpdated(): void;
  /** The model and result held before the last accepted roll forward. */
  previousRoll: EvaluatedModel | null;
  /** The evaluated model now shown; null before an evaluation. */
  current: EvaluatedModel | null;
  density: "compact" | "comfortable";
}) {
  const [newForecastNode, setNewForecastNode] = useState("");
  const forecastNodes = Object.entries(model.nodes).filter(
    ([, node]) => node.forecast,
  );
  const forecastCandidates = Object.entries(model.nodes).filter(
    ([, node]) => node.node_type === "mixed" && !node.forecast,
  );
  const addForecast = async () => {
    if (!newForecastNode || !model.nodes[newForecastNode]) return;
    const next = structuredClone(model);
    next.nodes[newForecastNode].forecast = {
      method: "forward_fill",
      params: {},
    };
    const message = await onCommit(JSON.stringify(next));
    if (!message) setNewForecastNode("");
  };
  return (
    <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_20rem]">
      <div className="space-y-4">
        <div>
          <h3 className="text-lg font-semibold">Forecast assumptions</h3>
          <p className="text-sm text-muted-foreground">
            Choose a forecast method and edit its assumptions. Rates are
            decimals; levels use exact model units. WASM calculates the
            projected values.
          </p>
        </div>
        {forecastNodes.map(([id, node]) => (
          <ForecastEditor
            key={`${revision}:${id}`}
            model={model}
            nodeId={id}
            node={node}
            onApply={(forecast) => {
              const next = structuredClone(model);
              next.nodes[id].forecast = forecast;
              return onCommit(JSON.stringify(next));
            }}
          />
        ))}
        {forecastNodes.length === 0 && (
          <p className="text-sm text-muted-foreground">
            This supplied model has no forecast driver nodes.
          </p>
        )}
        {forecastCandidates.length > 0 && (
          <div className="rounded-lg border border-border bg-card p-4">
            <h4 className="font-semibold">Add a forecast driver</h4>
            <p className="mt-1 text-xs text-muted-foreground">
              Start with carry forward, then choose the projection method and
              assumptions above.
            </p>
            <div className="mt-3 flex flex-wrap items-end gap-2">
              <div className="min-w-48 flex-1">
                <EnumField
                  label="Line item"
                  layout="select"
                  value={newForecastNode}
                  onValueChange={setNewForecastNode}
                  options={forecastCandidates.map(([id, node]) => ({
                    value: id,
                    label: node.name ?? id,
                  }))}
                />
              </div>
              <Button
                type="button"
                size="sm"
                disabled={!newForecastNode}
                onClick={() => void addForecast()}
              >
                Add driver
              </Button>
            </div>
          </div>
        )}
      </div>
      <aside className="h-fit space-y-3 rounded-lg border border-border bg-card p-4">
        <h3 className="font-semibold">Roll forward actuals</h3>
        {nextForecastId ? (
          <>
            <p className="text-sm">
              Next forecast period:{" "}
              <strong className="font-mono">{nextForecastId}</strong>
            </p>
            <p className="text-xs text-muted-foreground">
              Bring in the next complete model with reported values and updated
              forecast assumptions. The workbench validates it and refreshes
              every statement and ratio.
            </p>
            {onRollForward && (
              <Button
                type="button"
                disabled={rolling || !ready}
                onClick={onRollForward}
              >
                {rolling
                  ? "Loading actuals…"
                  : `Roll forward to ${nextForecastId}`}
              </Button>
            )}
          </>
        ) : (
          <p className="text-sm text-muted-foreground">
            All supplied periods are actual. Load a model with forecast periods
            to continue.
          </p>
        )}
        <Button type="button" variant="link" size="sm" onClick={onLoadUpdated}>
          Load updated financials
        </Button>
      </aside>
      {previousRoll && current && (
        <div className="lg:col-span-2">
          <RollForwardReview
            previous={previousRoll}
            current={current}
            density={density}
          />
        </div>
      )}
    </div>
  );
}
