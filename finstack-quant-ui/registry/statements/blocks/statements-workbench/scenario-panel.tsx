"use client";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { formatRawMoney } from "@/lib/finstack/format/format";
import type { StatementScenarioResults } from "@/workers/finstack-contract";
import {
  adaptStatementResult,
  reportStatementCell,
  statementRows,
  type StatementCell,
} from "../../components/statement-grid/projection";
import { ForecastEditor, type ForecastSpecWire } from "./forecast-editor";

type Value = NonNullable<
  FinancialModelSpecWire["nodes"][string]["values"]
>[string];
interface ScenarioDraft {
  name: string;
  parent: string;
  /** Exact value text per node, applied to every forecast period. */
  overrides: Record<string, string>;
  /** Whole native forecast spec per node. */
  forecasts: Record<string, ForecastSpecWire>;
  /** Host-supplied per-period overrides, passed through unchanged. */
  periodOverrides?: unknown;
}
interface ScenarioSetWire {
  scenarios: Record<
    string,
    {
      parent?: string;
      overrides?: Record<string, Value>;
      period_overrides?: unknown;
      forecast_overrides?: Record<string, ForecastSpecWire>;
    }
  >;
}
const inputClass =
  "w-full rounded-md border border-border bg-background px-2 py-1.5 text-sm";
const cardClass = "rounded-lg border border-border bg-card p-4";

function draftsFrom(json: string | undefined): ScenarioDraft[] {
  if (json === undefined)
    return [{ name: "base", parent: "", overrides: {}, forecasts: {} }];
  const set = JSON.parse(json) as ScenarioSetWire;
  return Object.entries(set.scenarios).map(([name, definition]) => ({
    name,
    parent: definition.parent ?? "",
    overrides: Object.fromEntries(
      Object.entries(definition.overrides ?? {}).map(([id, value]) => [
        id,
        typeof value === "number" ? String(value) : value.amount,
      ]),
    ),
    forecasts: { ...definition.forecast_overrides },
    periodOverrides: definition.period_overrides,
  }));
}

/** Native ScenarioSet JSON; exact value text keeps the line's unit and currency. */
function scenarioSetJson(
  model: FinancialModelSpecWire,
  drafts: ScenarioDraft[],
): string {
  const names = drafts.map((draft) => draft.name.trim());
  if (names.some((name) => !name)) throw new Error("Name every scenario.");
  if (new Set(names).size !== names.length)
    throw new Error("Scenario names must be unique.");
  const scenarios: ScenarioSetWire["scenarios"] = {};
  for (const draft of drafts) {
    const overrides: Record<string, Value> = {};
    for (const [id, text] of Object.entries(draft.overrides)) {
      const label = model.nodes[id]?.name ?? id;
      const type = model.nodes[id]?.value_type;
      if (type?.type === "monetary") {
        if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(text.trim()))
          throw new Error(`${label}: enter a decimal amount in base units.`);
        overrides[id] = { amount: text.trim(), currency: type.currency };
      } else {
        const number = Number(text);
        if (!text.trim() || !Number.isFinite(number))
          throw new Error(`${label}: enter a finite number.`);
        overrides[id] = number;
      }
    }
    scenarios[draft.name.trim()] = {
      ...(draft.parent ? { parent: draft.parent } : {}),
      ...(Object.keys(overrides).length ? { overrides } : {}),
      ...(draft.periodOverrides !== undefined
        ? { period_overrides: draft.periodOverrides }
        : {}),
      ...(Object.keys(draft.forecasts).length
        ? { forecast_overrides: draft.forecasts }
        : {}),
    };
  }
  return JSON.stringify({ scenarios });
}

function moneyCell(
  periodId: string,
  value: Parameters<typeof formatRawMoney>[0] | undefined,
): StatementCell {
  return value
    ? { periodId, present: true, value, text: formatRawMoney(value) }
    : { periodId, present: false, value: undefined, text: "—" };
}

/** Define named scenarios over the current model, run them natively and compare the results. */
export function ScenarioPanel({
  model,
  modelJson,
  nodeIds,
  marketJson,
  asOf,
  activePeriod,
  displayMode,
  link,
  defaultScenarioSetJson,
}: {
  model: FinancialModelSpecWire;
  /** Canonical model the scenarios override. */
  modelJson: string;
  nodeIds: string[];
  marketJson?: string;
  asOf?: string;
  activePeriod: string | null;
  displayMode: "exact" | "millions";
  link?: LinkedSelection;
  /** Optional host-supplied native ScenarioSet JSON. */
  defaultScenarioSetJson?: string;
}) {
  const { client, status } = useFinstack();
  const [drafts, setDrafts] = useState<ScenarioDraft[]>(() =>
    draftsFrom(defaultScenarioSetJson),
  );
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [run, setRun] = useState<{
    modelJson: string;
    setJson: string;
    results: StatementScenarioResults;
  } | null>(null);
  const [shown, setShown] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const results = run?.modelJson === modelJson ? run.results : null;
  const names = useMemo(() => (results ? Object.keys(results) : []), [results]);
  const shownName = shown && names.includes(shown) ? shown : names[0];
  const valueNodes = nodeIds.filter(
    (id) => model.nodes[id] && model.nodes[id].node_type !== "calculated",
  );
  const update = (index: number, change: Partial<ScenarioDraft>) =>
    setDrafts(
      drafts.map((draft, i) => (i === index ? { ...draft, ...change } : draft)),
    );
  const comparison = useMemo(() => {
    if (!results || !activePeriod) return null;
    const rows = Object.fromEntries(
      names.map((name) => [name, adaptStatementResult(results[name])]),
    );
    const lines = nodeIds.map((id) => ({
      id,
      name: model.nodes[id]?.name ?? id,
      cells: names.map(
        (name) =>
          statementRows(model, rows[name]).find((row) => row.nodeId === id)
            ?.cells[activePeriod],
      ),
    }));
    const totals = names.map(
      (name) => rows[name].cs_cashflows?.totals?.[activePeriod],
    );
    const capital = totals.some(Boolean)
      ? [
          {
            id: "cs.debt_balance",
            name: "Debt balance (capital structure)",
            cells: totals.map((total) =>
              moneyCell(activePeriod, total?.debt_balance),
            ),
          },
          {
            id: "cs.interest_expense_cash",
            name: "Cash interest (capital structure)",
            cells: totals.map((total) =>
              moneyCell(activePeriod, total?.interest_expense_cash),
            ),
          },
        ]
      : [];
    return [...lines, ...capital];
  }, [results, names, activePeriod, nodeIds, model]);

  const runScenarios = async () => {
    if (!client || status !== "ready") return;
    setError(null);
    let setJson: string;
    try {
      setJson = scenarioSetJson(model, drafts);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      return;
    }
    setRunning(true);
    try {
      const value = await client.call("evaluateStatementScenarios", {
        modelJson,
        scenarioSetJson: setJson,
        marketJson,
        asOf,
      });
      setRun({ modelJson, setJson, results: value });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <h3 className="text-lg font-semibold">Scenarios</h3>
        <p className="text-sm text-muted-foreground">
          Override driver values and forecast methods per scenario. A child
          scenario inherits its parent and replaces the forecast of any line it
          overrides; explicit values still win over forecasts. WASM evaluates
          every scenario.{" "}
          {marketJson
            ? `The capital structure is priced from the supplied market as of ${asOf}.`
            : "Any capital structure uses contractual cashflows without market data, so floating-rate debt and hedges need a host market."}
        </p>
      </div>
      {drafts.map((draft, index) => {
        const others = drafts.filter((_, i) => i !== index);
        const unusedValue = valueNodes.filter((id) => !(id in draft.overrides));
        const unusedForecast = nodeIds.filter((id) => !(id in draft.forecasts));
        return (
          <section
            key={index}
            aria-label={`Scenario ${draft.name || index + 1}`}
            className={`${cardClass} space-y-4`}
          >
            <div className="grid gap-3 sm:grid-cols-[1fr_1fr_auto] sm:items-end">
              <label className="text-xs">
                Scenario name
                <input
                  className={`${inputClass} mt-1`}
                  value={draft.name}
                  onChange={(event) =>
                    update(index, { name: event.target.value })
                  }
                />
              </label>
              <label className="text-xs">
                Inherits from
                <select
                  className={`${inputClass} mt-1`}
                  value={draft.parent}
                  onChange={(event) =>
                    update(index, { parent: event.target.value })
                  }
                >
                  <option value="">No parent</option>
                  {others.map((other) => (
                    <option key={other.name} value={other.name}>
                      {other.name}
                    </option>
                  ))}
                </select>
              </label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={drafts.length === 1}
                onClick={() => setDrafts(drafts.filter((_, i) => i !== index))}
              >
                Remove scenario
              </Button>
            </div>
            <div className="space-y-2">
              <h4 className="text-sm font-semibold">Value overrides</h4>
              <p className="text-xs text-muted-foreground">
                Applied to every forecast period, in the line&apos;s exact base
                units.
              </p>
              {Object.entries(draft.overrides).map(([id, text]) => (
                <div key={id} className="flex flex-wrap items-end gap-2">
                  <label className="min-w-48 flex-1 text-xs">
                    {model.nodes[id]?.name ?? id}
                    <input
                      aria-label={`${draft.name} ${model.nodes[id]?.name ?? id} override`}
                      className={`${inputClass} mt-1 font-mono`}
                      inputMode="decimal"
                      value={text}
                      onChange={(event) =>
                        update(index, {
                          overrides: {
                            ...draft.overrides,
                            [id]: event.target.value,
                          },
                        })
                      }
                    />
                  </label>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      const { [id]: _, ...rest } = draft.overrides;
                      update(index, { overrides: rest });
                    }}
                  >
                    Remove
                  </Button>
                </div>
              ))}
              {unusedValue.length > 0 && (
                <select
                  aria-label={`${draft.name} add value override`}
                  className={`${inputClass} max-w-xs`}
                  value=""
                  onChange={(event) =>
                    update(index, {
                      overrides: {
                        ...draft.overrides,
                        [event.target.value]: "",
                      },
                    })
                  }
                >
                  <option value="">Add a value override…</option>
                  {unusedValue.map((id) => (
                    <option key={id} value={id}>
                      {model.nodes[id]?.name ?? id}
                    </option>
                  ))}
                </select>
              )}
            </div>
            <div className="space-y-2">
              <h4 className="text-sm font-semibold">Forecast overrides</h4>
              {Object.entries(draft.forecasts).map(([id, forecast]) => (
                <div key={id} className="space-y-1">
                  <ForecastEditor
                    key={`${revision}:${draft.name}:${id}`}
                    model={model}
                    nodeId={id}
                    node={{ ...model.nodes[id], forecast }}
                    applyLabel="Save to scenario"
                    onApply={async (next) => {
                      update(index, {
                        forecasts: { ...draft.forecasts, [id]: next },
                      });
                      return null;
                    }}
                  />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      const { [id]: _, ...rest } = draft.forecasts;
                      update(index, { forecasts: rest });
                    }}
                  >
                    Remove forecast override
                  </Button>
                </div>
              ))}
              {unusedForecast.length > 0 && (
                <select
                  aria-label={`${draft.name} add forecast override`}
                  className={`${inputClass} max-w-xs`}
                  value=""
                  onChange={(event) => {
                    const id = event.target.value;
                    setRevision((value) => value + 1);
                    update(index, {
                      forecasts: {
                        ...draft.forecasts,
                        [id]: model.nodes[id]?.forecast ?? {
                          method: "forward_fill",
                          params: {},
                        },
                      },
                    });
                  }}
                >
                  <option value="">Add a forecast override…</option>
                  {unusedForecast.map((id) => (
                    <option key={id} value={id}>
                      {model.nodes[id]?.name ?? id}
                    </option>
                  ))}
                </select>
              )}
            </div>
          </section>
        );
      })}
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={() =>
            setDrafts([
              ...drafts,
              {
                name: `scenario_${drafts.length + 1}`,
                parent: drafts[0]?.name ?? "",
                overrides: {},
                forecasts: {},
              },
            ])
          }
        >
          Add scenario
        </Button>
        <Button
          type="button"
          disabled={running || status !== "ready"}
          onClick={() => void runScenarios()}
        >
          {running ? "Evaluating scenarios…" : "Run scenarios"}
        </Button>
      </div>
      {error && (
        <p
          role="alert"
          className="rounded-md border border-error p-3 text-sm text-error"
        >
          {error}
        </p>
      )}
      {results && comparison && activePeriod && (
        <section aria-label="Scenario comparison" className="space-y-2">
          <h3 className="text-base font-semibold">
            Scenario comparison · {activePeriod}
          </h3>
          <div className="overflow-x-auto border-y border-foreground/70 bg-card">
            <table className="w-full min-w-max border-collapse text-sm tabular-nums">
              <caption className="sr-only">
                Scenario values for {activePeriod}
              </caption>
              <thead>
                <tr className="border-b border-foreground/50 text-xs">
                  <th
                    scope="col"
                    className="sticky left-0 bg-card px-3 py-2 text-left"
                  >
                    Line item
                  </th>
                  {names.map((name) => (
                    <th
                      key={name}
                      scope="col"
                      className="min-w-28 px-3 py-2 text-right"
                    >
                      {name}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {comparison.map((line) => (
                  <tr key={line.id} className="border-b border-border/60">
                    <th
                      scope="row"
                      className="sticky left-0 bg-card px-3 py-2 text-left font-normal"
                    >
                      {line.name}
                    </th>
                    {line.cells.map((cell, i) => (
                      <td
                        key={names[i]}
                        className="px-3 py-2 text-right font-mono text-xs"
                      >
                        {reportStatementCell(cell, displayMode)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
      {results && shownName && (
        <div className="space-y-3">
          <div
            className="flex flex-wrap gap-2"
            aria-label="Scenario statements"
          >
            {names.map((name) => (
              <button
                key={name}
                type="button"
                className={`rounded-md border px-3 py-1.5 text-xs ${shownName === name ? "border-primary bg-primary/10 font-semibold" : "border-border bg-card"}`}
                aria-pressed={shownName === name}
                onClick={() => setShown(name)}
              >
                {name}
              </button>
            ))}
          </div>
          <StatementGrid
            title={`${shownName} statements`}
            model={model}
            result={results[shownName]}
            link={link}
            nodeIds={nodeIds}
            displayMode={displayMode}
            showType={false}
            showRaw={false}
            activePeriod={activePeriod}
          />
        </div>
      )}
      {run && (
        <details className={cardClass}>
          <summary className="cursor-pointer font-semibold">
            Scenario set JSON
          </summary>
          <div className="mt-3">
            <JsonViewer
              label="Native scenario set"
              text={run.setJson}
              downloadName="scenario-set.json"
            />
          </div>
        </details>
      )}
    </div>
  );
}
