"use client";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { ForecastEditor } from "./forecast-editor";
import { MetricEditor } from "./metric-editor";
import { FinancialModelEditor } from "@/components/finstack/statements/components/financial-model-editor/financial-model-editor";
import { financialModelModule } from "@/components/finstack/statements/components/financial-model-editor/model";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { StatementChart } from "@/components/finstack/statements/components/statement-chart/statement-chart";
import { StatementExplanation } from "@/components/finstack/statements/components/statement-diagnostics/statement-explanation";
import { StatementChecks } from "@/components/finstack/statements/components/statement-diagnostics/statement-checks";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { useEvaluateStatement } from "@/hooks/statements/use-evaluate-statement/use-evaluate-statement";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  adaptStatementResult,
  displayStatementCell,
  statementCellKey,
  statementRows,
} from "../../components/statement-grid/projection";
import type { StatementRequest } from "@/workers/finstack-contract";

type Tab = "statements" | "forecast" | "ratios" | "checks" | "advanced";
const tabs: { id: Tab; label: string }[] = [
  { id: "statements", label: "Statements & editing" },
  { id: "forecast", label: "Forecast & roll forward" },
  { id: "ratios", label: "Metrics & ratios" },
  { id: "checks", label: "Checks" },
  { id: "advanced", label: "Advanced" },
];
const inputClass =
  "w-full rounded-md border border-border bg-background px-3 py-2 text-sm";
const cardClass = "rounded-lg border border-border bg-card p-4";

/** Review supplied financials, edit explicit values and forecast inputs, and evaluate through native WASM. Wrap in FinstackQueryProvider. */
export function StatementsWorkbench({
  defaultModelJson,
  marketJson,
  asOf,
  defaultCheckSuiteJson,
  selectedKey,
  onSelectedKeyChange,
  onModelAccepted,
  onRollForward,
}: {
  /** Complete canonical model supplied by the host or a financial-data service. Remount for a different document. */
  defaultModelJson: string;
  /** Both market JSON and ISO as-of must be supplied for market-aware evaluation. */
  marketJson?: string;
  asOf?: string;
  /** Optional canonical check suite supplied by the host. */
  defaultCheckSuiteJson?: string;
  selectedKey?: string | null;
  onSelectedKeyChange?: (key: string | null) => void;
  onModelAccepted?: (canonicalJson: string) => void;
  /** Host obtains the next complete model from its financial-data service. The returned JSON is validated and evaluated natively. */
  onRollForward?: (
    canonicalJson: string,
    nextPeriodId: string,
  ) => Promise<string>;
}) {
  const { client, status } = useFinstack();
  const link = useLinkedSelection({ selectedKey, onSelectedKeyChange });
  const [document, setDocument] = useState(defaultModelJson);
  const [importText, setImportText] = useState(defaultModelJson);
  const [request, setRequest] = useState<StatementRequest | null>(null);
  const [nodeIds, setNodeIds] = useState<string[]>([]);
  const [tab, setTab] = useState<Tab>("statements");
  const [displayMode, setDisplayMode] = useState<"exact" | "millions">(
    "millions",
  );
  const [editPeriod, setEditPeriod] = useState<string | null>(null);
  const [chartNode, setChartNode] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [rolling, setRolling] = useState(false);
  const [newForecastNode, setNewForecastNode] = useState("");
  const [revision, setRevision] = useState(0);
  const generation = useRef(0);
  const initialized = useRef(false);
  const marketPair = (marketJson === undefined) === (asOf === undefined);
  const model = useMemo(() => {
    try {
      return financialModelModule.codec.parse(
        document,
      ) as FinancialModelSpecWire;
    } catch {
      return null;
    }
  }, [document]);
  const commit = useCallback(
    async (input: string, preserveOrder = false) => {
      if (!client || status !== "ready")
        return "Statement runtime is not ready.";
      if (!marketPair) {
        const message = "Supply market JSON and the as-of date together.";
        setActionError(message);
        return message;
      }
      const token = ++generation.current;
      setActionError(null);
      try {
        const canonical = await client.call("validateStatementModel", input);
        const ids = await client.call("statementNodeIds", canonical);
        await client.call("evaluateStatement", {
          modelJson: canonical,
          marketJson,
          asOf,
        });
        if (token !== generation.current)
          return "A newer statement update superseded this one.";
        const sourceOrder = Object.keys(
          (JSON.parse(input) as FinancialModelSpecWire).nodes ?? {},
        );
        const present = new Set(ids);
        setDocument(canonical);
        setImportText(canonical);
        setNodeIds((previous) => {
          const order =
            preserveOrder && previous.length ? previous : sourceOrder;
          return [
            ...order.filter((id) => present.has(id)),
            ...ids.filter((id) => !order.includes(id)),
          ];
        });
        setRequest({ modelJson: canonical, marketJson, asOf });
        setRevision((value) => value + 1);
        onModelAccepted?.(canonical);
        return null;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (token === generation.current) setActionError(message);
        return message;
      }
    },
    [client, status, marketPair, marketJson, asOf, onModelAccepted],
  );
  useEffect(() => {
    if (initialized.current || status !== "ready" || !client) return;
    initialized.current = true;
    void commit(defaultModelJson);
  }, [client, status, commit, defaultModelJson]);

  const evaluation = useEvaluateStatement(request);
  const result = request?.modelJson === document ? evaluation.data : undefined;
  const resultJson = useMemo(
    () => (result ? serializeHost(result) : null),
    [result],
  );
  const rows = useMemo(() => {
    if (!result || !model) return [];
    try {
      return statementRows(model, adaptStatementResult(result));
    } catch {
      return [];
    }
  }, [model, result]);
  const sections = useMemo(() => {
    const ids = nodeIds.length ? nodeIds : Object.keys(model?.nodes ?? {});
    const tagged = (tag: string) =>
      ids.filter((id) => model?.nodes[id]?.tags?.includes(tag));
    const income = tagged("income_statement");
    const balance = ids.filter((id) =>
      model?.nodes[id]?.tags?.some(
        (tag) => tag === "balance_sheet" || tag === "debt",
      ),
    );
    const cash = tagged("cash_flow");
    const ratios = ids.filter((id) =>
      model?.nodes[id]?.tags?.some(
        (tag) => tag === "ratio" || tag === "metric",
      ),
    );
    const shown = new Set([...income, ...balance, ...cash, ...ratios]);
    return {
      income,
      balance,
      cash,
      ratios,
      other: ids.filter((id) => !shown.has(id)),
    };
  }, [model, nodeIds]);
  const selected = useMemo(() => {
    if (!model || !link.selectedKey) return null;
    for (const nodeId of Object.keys(model.nodes))
      for (const period of model.periods)
        if (statementCellKey(nodeId, period.id) === link.selectedKey)
          return { nodeId, periodId: period.id };
    return null;
  }, [model, link.selectedKey]);
  const activeNode =
    selected?.nodeId ?? sections.income[0] ?? nodeIds[0] ?? null;
  const activePeriod =
    selected?.periodId ??
    editPeriod ??
    model?.periods.findLast((period) => period.is_actual)?.id ??
    model?.periods[0]?.id ??
    null;
  const headline = rows.filter((row) =>
    model?.nodes[row.nodeId]?.tags?.some(
      (tag) => tag === "headline" || tag === "covenant",
    ),
  );
  const chartId = sections.ratios.includes(chartNode ?? "")
    ? chartNode
    : sections.ratios[0];
  const entity =
    typeof model?.meta?.entity === "string"
      ? model.meta.entity
      : (model?.id ?? "Financial statements");
  const currency =
    typeof model?.meta?.reporting_currency === "string"
      ? model.meta.reporting_currency
      : null;
  const forecastNodes = model
    ? Object.entries(model.nodes).filter(([, node]) => node.forecast)
    : [];
  const forecastCandidates = model
    ? Object.entries(model.nodes).filter(
        ([, node]) => node.node_type === "mixed" && !node.forecast,
      )
    : [];
  const nextForecast = model?.periods.find((period) => !period.is_actual);
  const rollForward = async () => {
    if (!onRollForward || !request || !nextForecast) return;
    const token = ++generation.current;
    setRolling(true);
    setActionError(null);
    try {
      const updated = await onRollForward(request.modelJson, nextForecast.id);
      const rolled = financialModelModule.codec.parse(
        updated,
      ) as FinancialModelSpecWire;
      if (
        !rolled.periods.some(
          (period) => period.id === nextForecast.id && period.is_actual,
        )
      )
        throw new Error(
          `Updated financials must mark ${nextForecast.id} actual.`,
        );
      if (token === generation.current) await commit(updated);
    } catch (error) {
      if (token === generation.current)
        setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setRolling(false);
    }
  };
  const choosePeriod = (id: string) => {
    link.clear();
    setEditPeriod(id);
  };
  const saveCell = async (
    nodeId: string,
    periodId: string,
    value: string | null,
  ) => {
    if (!model) return "No statement model is loaded.";
    const next = structuredClone(model);
    const node = next.nodes[nodeId];
    if (!node || node.node_type === "calculated")
      return "This line is calculated from its drivers.";
    const values = { ...node.values };
    if (value === null) delete values[periodId];
    else if (!value.trim()) return "Enter an exact value.";
    else if (node.value_type?.type === "monetary") {
      if (!/^[+-]?(?:\d+\.?\d*|\.\d+)$/.test(value.trim()))
        return "Enter a decimal amount in base units.";
      values[periodId] = {
        amount: value.trim(),
        currency: node.value_type.currency,
      };
    } else {
      const number = Number(value);
      if (!Number.isFinite(number)) return "Enter a finite number.";
      values[periodId] = number;
    }
    node.values = values;
    return commit(JSON.stringify(next), true);
  };
  const addForecast = async () => {
    if (!model || !newForecastNode || !model.nodes[newForecastNode]) return;
    const next = structuredClone(model);
    next.nodes[newForecastNode].forecast = {
      method: "forward_fill",
      params: {},
    };
    const message = await commit(JSON.stringify(next), true);
    if (!message) setNewForecastNode("");
  };

  return (
    <section
      aria-label="Statements workbench"
      className="space-y-5 rounded-xl border border-border bg-background p-4 font-sans text-foreground md:p-6"
    >
      <header className="space-y-4 border-b border-border pb-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-widest text-muted-foreground">
              Credit analysis · financial statements
            </p>
            <h2 className="mt-1 text-2xl font-semibold">{entity}</h2>
            <p className="text-sm text-muted-foreground">
              {model?.id}
              {currency ? ` · ${currency}` : ""}
              {model?.periods.length
                ? ` · ${model.periods.length} reporting periods`
                : ""}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <span
              className="rounded-full border border-border px-3 py-1 text-xs"
              role="status"
            >
              {evaluation.isFetching
                ? "Evaluating…"
                : result
                  ? "Evaluated"
                  : status === "ready"
                    ? "Loading financials…"
                    : "Starting runtime…"}
            </span>
          </div>
        </div>
        {model && (
          <div className="flex flex-wrap gap-2" aria-label="Reporting periods">
            {model.periods.map((period) => (
              <button
                key={period.id}
                type="button"
                onClick={() => choosePeriod(period.id)}
                className={`rounded-md border px-3 py-1.5 text-xs ${activePeriod === period.id ? "border-primary bg-primary/10 font-semibold" : "border-border bg-card"}`}
                aria-pressed={activePeriod === period.id}
              >
                <span className="font-mono">{period.id}</span>{" "}
                <span className="ml-1 text-muted-foreground">
                  {period.is_actual ? "Historical" : "Forecast"}
                </span>
              </button>
            ))}
          </div>
        )}
      </header>
      {actionError && (
        <p
          role="alert"
          className="rounded-md border border-error p-3 text-sm text-error"
        >
          {actionError}
        </p>
      )}
      {evaluation.error && (
        <p
          role="alert"
          className="rounded-md border border-error p-3 text-sm text-error"
        >
          {evaluation.error.message}
        </p>
      )}
      <div
        className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4"
        aria-label="Headline metrics"
      >
        {headline.slice(0, 4).map((row) => (
          <div className={cardClass} key={row.nodeId}>
            <p className="text-xs text-muted-foreground">{row.name}</p>
            <p className="mt-2 text-lg font-semibold tabular-nums">
              {activePeriod
                ? displayStatementCell(row.cells[activePeriod], displayMode)
                : "—"}
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {activePeriod ?? ""} ·{" "}
              {row.valueType.startsWith("Monetary")
                ? displayMode === "millions"
                  ? "display in millions"
                  : "exact native value"
                : "native ratio"}
            </p>
          </div>
        ))}
      </div>
      <div
        className="flex flex-wrap items-center justify-end gap-2 text-xs"
        aria-label="Display units"
      >
        <span className="text-muted-foreground">Display units</span>
        <button
          type="button"
          className={`rounded-md border px-2 py-1 ${displayMode === "millions" ? "border-primary bg-primary/10 font-semibold" : "border-border"}`}
          aria-pressed={displayMode === "millions"}
          onClick={() => setDisplayMode("millions")}
        >
          Millions
        </button>
        <button
          type="button"
          className={`rounded-md border px-2 py-1 ${displayMode === "exact" ? "border-primary bg-primary/10 font-semibold" : "border-border"}`}
          aria-pressed={displayMode === "exact"}
          onClick={() => setDisplayMode("exact")}
        >
          Exact
        </button>
      </div>
      <nav
        className="flex flex-wrap gap-1 border-b border-border"
        aria-label="Statement workspace"
      >
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`rounded-t-md px-3 py-2 text-sm ${tab === item.id ? "border-b-2 border-primary font-semibold" : "text-muted-foreground hover:text-foreground"}`}
            onClick={() => setTab(item.id)}
            aria-current={tab === item.id ? "page" : undefined}
          >
            {item.label}
          </button>
        ))}
      </nav>
      {!model && <p role="alert">The supplied model is not valid JSON.</p>}
      {!result && !evaluation.isFetching && !actionError && (
        <p className={cardClass} role="status">
          Loading supplied financial statements…
        </p>
      )}
      {model && result && resultJson && tab === "statements" && (
        <div className="space-y-6">
          <p className="text-xs text-muted-foreground">
            Select a period above to review it. Click a reported or forecast
            value to edit in exact units; calculated lines update from their
            drivers.
          </p>
          <div className="min-w-0 space-y-8">
            {sections.income.length > 0 && (
              <StatementGrid
                title="Income statement"
                model={model}
                result={result}
                link={link}
                nodeIds={sections.income}
                displayMode={displayMode}
                showType={false}
                showRaw={false}
                activePeriod={activePeriod}
                onEditValue={saveCell}
              />
            )}
            {sections.balance.length > 0 && (
              <StatementGrid
                title="Balance sheet & debt"
                model={model}
                result={result}
                link={link}
                nodeIds={sections.balance}
                displayMode={displayMode}
                showType={false}
                showRaw={false}
                activePeriod={activePeriod}
                onEditValue={saveCell}
              />
            )}
            {sections.cash.length > 0 && (
              <StatementGrid
                title="Cash flow"
                model={model}
                result={result}
                link={link}
                nodeIds={sections.cash}
                displayMode={displayMode}
                showType={false}
                showRaw={false}
                activePeriod={activePeriod}
                onEditValue={saveCell}
              />
            )}
            {sections.other.length > 0 && (
              <StatementGrid
                title="Other lines"
                model={model}
                result={result}
                link={link}
                nodeIds={sections.other}
                displayMode={displayMode}
                showType={false}
                showRaw={false}
                activePeriod={activePeriod}
                onEditValue={saveCell}
              />
            )}
          </div>
          {activeNode && activePeriod && (
            <details className={cardClass}>
              <summary className="cursor-pointer font-semibold">
                Explain selected value
              </summary>
              <div className="mt-3">
                <StatementExplanation
                  request={{
                    modelJson: request!.modelJson,
                    resultsJson: resultJson,
                    nodeId: activeNode,
                    period: activePeriod,
                  }}
                />
              </div>
            </details>
          )}
        </div>
      )}
      {model && tab === "forecast" && (
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
                onApply={(json) => commit(json, true)}
              />
            ))}
            {forecastNodes.length === 0 && (
              <p className={cardClass}>
                This supplied model has no forecast driver nodes.
              </p>
            )}
            {forecastCandidates.length > 0 && (
              <div className={cardClass}>
                <h4 className="font-semibold">Add a forecast driver</h4>
                <p className="mt-1 text-xs text-muted-foreground">
                  Start with carry forward, then choose the projection method
                  and assumptions above.
                </p>
                <div className="mt-3 flex flex-wrap items-end gap-2">
                  <label className="min-w-48 flex-1 text-xs">
                    Line item
                    <select
                      className={`${inputClass} mt-1`}
                      value={newForecastNode}
                      onChange={(event) =>
                        setNewForecastNode(event.target.value)
                      }
                    >
                      <option value="">Select a line…</option>
                      {forecastCandidates.map(([id, node]) => (
                        <option key={id} value={id}>
                          {node.name ?? id}
                        </option>
                      ))}
                    </select>
                  </label>
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
          <aside className={`${cardClass} h-fit space-y-3`}>
            <h3 className="font-semibold">Roll forward actuals</h3>
            {nextForecast ? (
              <>
                <p className="text-sm">
                  Next forecast period:{" "}
                  <strong className="font-mono">{nextForecast.id}</strong>
                </p>
                <p className="text-xs text-muted-foreground">
                  Bring in the next complete model with reported values and
                  updated forecast assumptions. The workbench validates it and
                  refreshes every statement and ratio.
                </p>
                {onRollForward && (
                  <Button
                    type="button"
                    disabled={rolling || status !== "ready"}
                    onClick={() => void rollForward()}
                  >
                    {rolling
                      ? "Loading actuals…"
                      : `Roll forward to ${nextForecast.id}`}
                  </Button>
                )}
              </>
            ) : (
              <p className="text-sm text-muted-foreground">
                All supplied periods are actual. Load a model with forecast
                periods to continue.
              </p>
            )}
            <button
              type="button"
              className="text-sm text-primary underline"
              onClick={() => setTab("advanced")}
            >
              Load updated financials
            </button>
          </aside>
        </div>
      )}
      {model && result && tab === "ratios" && (
        <div className="space-y-5">
          <MetricEditor model={model} onApply={(json) => commit(json, true)} />
          <p className="text-sm text-muted-foreground">
            Every value below is calculated from the current financials by WASM.
          </p>
          {sections.ratios.length > 0 ? (
            <>
              <StatementGrid
                title="Metrics, margins & covenant ratios"
                model={model}
                result={result}
                link={link}
                nodeIds={sections.ratios}
                displayMode={displayMode}
                showType={false}
                showRaw={false}
                activePeriod={activePeriod}
              />
              <div className={cardClass}>
                <label className="block text-sm">
                  Trend
                  <select
                    className={`${inputClass} mt-1 max-w-md`}
                    value={chartId ?? ""}
                    onChange={(event) => setChartNode(event.target.value)}
                  >
                    {sections.ratios.map((id) => (
                      <option key={id} value={id}>
                        {model.nodes[id]?.name ?? id}
                      </option>
                    ))}
                  </select>
                </label>
                {chartId && (
                  <StatementChart
                    model={model}
                    result={result}
                    nodeId={chartId}
                    link={link}
                  />
                )}
              </div>
            </>
          ) : (
            <p className={cardClass}>
              No calculated metrics are supplied yet. Add one above.
            </p>
          )}
        </div>
      )}
      {model && result && resultJson && tab === "checks" && (
        <div className="max-w-5xl">
          <StatementChecks
            key={revision}
            modelJson={request!.modelJson}
            resultsJson={resultJson}
            defaultConfigJson={defaultCheckSuiteJson}
            link={link}
          />
        </div>
      )}
      {model && tab === "advanced" && (
        <div className="space-y-6">
          <div className={cardClass}>
            <h3 className="font-semibold">Load updated financials</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              Paste the complete canonical FinancialModelSpec JSON returned by
              your data provider or parsing service.
            </p>
            <label className="mt-3 block text-xs">
              Model JSON
              <textarea
                className={`${inputClass} mt-1 min-h-36 font-mono`}
                value={importText}
                onChange={(event) => setImportText(event.target.value)}
                spellCheck={false}
              />
            </label>
            <Button
              type="button"
              className="mt-3"
              disabled={status !== "ready"}
              onClick={() => void commit(importText)}
            >
              Load & evaluate
            </Button>
          </div>
          <details className={cardClass}>
            <summary className="cursor-pointer font-semibold">
              Model structure and formulas
            </summary>
            <div className="mt-4">
              <FinancialModelEditor
                key={revision}
                defaultJson={document}
                onSubmit={async (json) => {
                  await commit(json, true);
                }}
              />
            </div>
          </details>
          <details className={cardClass}>
            <summary className="cursor-pointer font-semibold">
              Canonical model and native result
            </summary>
            <div className="mt-3 space-y-4">
              <JsonViewer
                label="Canonical model JSON"
                text={document}
                downloadName="financial-model.json"
              />
              {resultJson && (
                <JsonViewer
                  label="Native statement result"
                  text={resultJson}
                  downloadName="statement-result.json"
                />
              )}
            </div>
          </details>
        </div>
      )}
    </section>
  );
}
