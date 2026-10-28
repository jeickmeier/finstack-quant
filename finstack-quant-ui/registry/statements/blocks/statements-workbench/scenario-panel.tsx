"use client";
import { useId, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  useEvaluateStatementScenarios,
  type StatementScenarioRequest,
} from "@/hooks/statements/use-evaluate-statement-scenarios/use-evaluate-statement-scenarios";
import type { LinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import schema from "@/lib/finstack/generated/schemas/scenario_set.json";
import type { ScenarioSetWire } from "@/lib/finstack/generated/types/scenario_set";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import { formatRawMoney } from "@/lib/finstack/format/format";
import {
  adaptStatementResult,
  reportStatementCell,
  statementRows,
  type StatementCell,
} from "../../components/statement-grid/projection";
import { ForecastEditor, type ForecastSpecWire } from "./forecast-editor";

const scenarioSetCodec = createWireCodec(schema);
type ScenarioDefinition = ScenarioSetWire["scenarios"][string];
type Value = NonNullable<ScenarioDefinition["overrides"]>[string];
interface ScenarioDraft {
  name: string;
  parent: string;
  /** Exact value text per node, applied to every forecast period. */
  overrides: Record<string, string>;
  /** Whole native forecast spec per node. */
  forecasts: Record<string, ForecastSpecWire>;
  /** Host-supplied per-period overrides, passed through unchanged. */
  periodOverrides?: ScenarioDefinition["period_overrides"];
}
/** One compared line: its name and the active-period cell of each scenario, in result order. */
interface ComparisonLine {
  id: string;
  name: string;
  cells: (StatementCell | undefined)[];
}

/** First failing wire path and its reason from a codec ZodError issue or wire TypeError. */
function codecFailure(error: unknown): {
  path: PropertyKey[];
  message: string;
} {
  const issue = (
    error as {
      issues?: { path: PropertyKey[]; message: string; code?: string }[];
    }
  ).issues?.[0];
  if (!issue)
    return {
      path: (error as { path?: PropertyKey[] }).path ?? [],
      // A wire TypeError message already starts with its path.
      message: error instanceof Error ? error.message : String(error),
    };
  return {
    path: issue.path,
    // A custom issue carries the wire TypeError text, path included.
    message:
      issue.code === "custom"
        ? issue.message
        : `/${issue.path.join("/")}: ${issue.message}`,
  };
}

/** Drafts from native ScenarioSet JSON; the codec validates it and keeps wide integers exact. */
function draftsFrom(json: string | undefined): ScenarioDraft[] {
  if (json === undefined)
    return [{ name: "base", parent: "", overrides: {}, forecasts: {} }];
  const set = scenarioSetCodec.parse(json) as ScenarioSetWire;
  return Object.entries(set.scenarios).map(([name, definition]) => ({
    name,
    parent: definition.parent ?? "",
    overrides: Object.fromEntries(
      Object.entries(definition.overrides ?? {}).map(([id, value]) => [
        id,
        typeof value === "object" ? value.amount : String(value),
      ]),
    ),
    forecasts: { ...definition.forecast_overrides },
    periodOverrides: definition.period_overrides,
  }));
}

/**
 * Native ScenarioSet JSON; exact value text keeps the line's unit and currency.
 * The generated codec enforces the Rust contract, including the decimal pattern
 * of monetary overrides, and names the failing line.
 */
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
  try {
    return scenarioSetCodec.stringify({ scenarios });
  } catch (cause) {
    const { path, message } = codecFailure(cause);
    const [root, , field, id] = path;
    if (
      root === "scenarios" &&
      field === "overrides" &&
      typeof id === "string"
    ) {
      const label = model.nodes[id]?.name ?? id;
      throw new Error(
        model.nodes[id]?.value_type?.type === "monetary"
          ? `${label}: enter a decimal amount in base units.`
          : `${label}: enter a finite number.`,
      );
    }
    throw new Error(message);
  }
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
  density,
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
  /** Density of the comparison table, statement grid and JSON viewer; undefined inherits the surrounding surface. */
  density?: "compact" | "comfortable";
}) {
  const cardId = useId();
  // Invalid host JSON starts from the base draft and shows why, instead of failing the panel.
  const [initial] = useState(() => {
    try {
      return { drafts: draftsFrom(defaultScenarioSetJson), error: null };
    } catch (cause) {
      return {
        drafts: draftsFrom(undefined),
        error: codecFailure(cause).message,
      };
    }
  });
  const [drafts, setDrafts] = useState<ScenarioDraft[]>(initial.drafts);
  const [error, setError] = useState<string | null>(initial.error);
  const [submitted, setSubmitted] = useState<StatementScenarioRequest | null>(
    null,
  );
  const request = submitted?.modelJson === modelJson ? submitted : null;
  const scenarios = useEvaluateStatementScenarios(request);
  const results = scenarios.data ?? null;
  const failure = error ?? scenarios.error?.message;
  const [shown, setShown] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const names = useMemo(() => (results ? Object.keys(results) : []), [results]);
  const shownName = shown && names.includes(shown) ? shown : names[0];
  const valueNodes = nodeIds.filter(
    (id) => model.nodes[id] && model.nodes[id].node_type !== "calculated",
  );
  const update = (index: number, change: Partial<ScenarioDraft>) =>
    setDrafts(
      drafts.map((draft, i) => (i === index ? { ...draft, ...change } : draft)),
    );
  const comparison = useMemo((): ComparisonLine[] | null => {
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

  const runScenarios = () => {
    setError(null);
    try {
      setSubmitted({
        modelJson,
        scenarioSetJson: scenarioSetJson(model, drafts),
        marketJson,
        asOf,
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
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
        // Repeated override controls name their scenario in the accessible description only.
        const scenarioId = `${cardId}-${index}`;
        return (
          <section
            key={index}
            aria-label={`Scenario ${draft.name || index + 1}`}
            className="space-y-4 rounded-lg border border-border bg-card p-4"
          >
            <span id={scenarioId} className="sr-only">
              Scenario {draft.name || index + 1}
            </span>
            <div className="grid gap-3 sm:grid-cols-[1fr_1fr_auto] sm:items-end">
              <FieldFrame label="Scenario name">
                {(control) => (
                  <Input
                    {...control}
                    value={draft.name}
                    onChange={(event) =>
                      update(index, { name: event.target.value })
                    }
                  />
                )}
              </FieldFrame>
              {/* Radio, not select: a select maps the empty "No parent" value to its placeholder. */}
              <EnumField
                label="Inherits from"
                layout="radio"
                value={draft.parent}
                onValueChange={(parent) => update(index, { parent })}
                options={[
                  { value: "", label: "No parent" },
                  ...others
                    .filter((other) => other.name)
                    .map((other) => ({ value: other.name, label: other.name })),
                ]}
              />
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
              {Object.entries(draft.overrides).map(([id, text]) => {
                const label = model.nodes[id]?.name ?? id;
                const setText = (value: string) =>
                  update(index, {
                    overrides: { ...draft.overrides, [id]: value },
                  });
                return (
                  <div key={id} className="flex flex-wrap items-end gap-2">
                    <div className="min-w-48 flex-1">
                      {model.nodes[id]?.value_type?.type === "monetary" ? (
                        <DecimalInput
                          label={label}
                          describedBy={scenarioId}
                          value={text}
                          onValueChange={setText}
                        />
                      ) : (
                        <FieldFrame label={label} describedBy={scenarioId}>
                          {(control) => (
                            <Input
                              {...control}
                              inputMode="decimal"
                              value={text}
                              onChange={(event) => setText(event.target.value)}
                            />
                          )}
                        </FieldFrame>
                      )}
                    </div>
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
                );
              })}
              {unusedValue.length > 0 && (
                <div className="max-w-xs">
                  <EnumField
                    label="Add value override"
                    describedBy={scenarioId}
                    layout="select"
                    value=""
                    onValueChange={(id) =>
                      update(index, {
                        overrides: { ...draft.overrides, [id]: "" },
                      })
                    }
                    options={unusedValue.map((id) => ({
                      value: id,
                      label: model.nodes[id]?.name ?? id,
                    }))}
                  />
                </div>
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
                <div className="max-w-xs">
                  <EnumField
                    label="Add forecast override"
                    describedBy={scenarioId}
                    layout="select"
                    value=""
                    onValueChange={(id) => {
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
                    options={unusedForecast.map((id) => ({
                      value: id,
                      label: model.nodes[id]?.name ?? id,
                    }))}
                  />
                </div>
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
          disabled={scenarios.isFetching || scenarios.workerStatus !== "ready"}
          onClick={runScenarios}
        >
          {scenarios.isFetching ? "Evaluating scenarios…" : "Run scenarios"}
        </Button>
      </div>
      {failure && (
        <p
          role="alert"
          className="rounded-md border border-error p-3 text-sm text-error"
        >
          {failure}
        </p>
      )}
      {results && comparison && activePeriod && (
        <section aria-label="Scenario comparison" className="space-y-2">
          <h3 className="text-base font-semibold">
            Scenario comparison · {activePeriod}
          </h3>
          <FinstackTable
            caption={`Scenario values for ${activePeriod}`}
            captionVisibility="sr-only"
            data={comparison}
            getRowId={(line) => line.id}
            columns={[
              {
                id: "line",
                header: "Line item",
                accessorFn: (line: ComparisonLine) => line.name,
                // Row headers announce each scenario value with its line item.
                meta: { className: "", rowHeader: true },
              },
              // Index-based ids keep a scenario named "line" distinct from the line column.
              ...names.map((name, i) => ({
                id: `scenario-${i}`,
                header: name,
                accessorFn: (line: ComparisonLine) =>
                  reportStatementCell(line.cells[i], displayMode),
                meta: { className: "text-right font-mono text-xs" },
              })),
            ]}
            density={density}
          />
        </section>
      )}
      {results && shownName && (
        <div className="space-y-3">
          <div
            role="group"
            aria-label="Scenario statements"
            className="flex flex-wrap gap-2"
          >
            {names.map((name) => (
              <Button
                key={name}
                type="button"
                variant="ghost"
                size="sm"
                aria-pressed={shownName === name}
                onClick={() => setShown(name)}
              >
                {name}
              </Button>
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
            density={density}
          />
        </div>
      )}
      {request && (
        <details>
          <summary>Scenario set JSON</summary>
          <div className="mt-3">
            <JsonViewer
              label="Native scenario set"
              text={request.scenarioSetJson}
              downloadName="scenario-set.json"
              density={density}
            />
          </div>
        </details>
      )}
    </div>
  );
}
