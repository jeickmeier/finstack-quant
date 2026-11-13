"use client";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useIsFetching, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScenarioPanel } from "./scenario-panel";
import { ValuationPanel } from "./valuation-panel";
import { SensitivityPanel } from "./sensitivity-panel";
import { MonteCarloPanel } from "./monte-carlo-panel";
import { CovenantsPanel } from "./covenants-panel";
import { StatementsPanel, type StatementSections } from "./statements-panel";
import { ForecastPanel } from "./forecast-panel";
import { RatiosPanel } from "./ratios-panel";
import { AdvancedPanel } from "./advanced-panel";
import type { EvaluatedModel } from "./roll-forward-review";
import {
  annualRollupSpecJson,
  hostRollupMethods,
  type RollupMethod,
} from "./rollup";
import { financialModelModule } from "@/components/finstack/statements/components/financial-model-editor/model";
import { StatementChecks } from "@/components/finstack/statements/components/statement-diagnostics/statement-checks";
import { SurfaceContext } from "@/components/finstack/shared/primitives/surface/surface";
import {
  evaluateStatementOptions,
  useEvaluateStatement,
} from "@/hooks/statements/use-evaluate-statement/use-evaluate-statement";
import { useStatementModelValidator } from "@/hooks/statements/use-statement-model-validator/use-statement-model-validator";
import {
  useDefaultRollupSpec,
  useRollupStatement,
} from "@/hooks/statements/use-rollup-statement/use-rollup-statement";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  adaptStatementResult,
  displayStatementCell,
  statementCellKey,
  statementRows,
} from "../../components/statement-grid/projection";
import type {
  StatementRequest,
  StatementRollupRequest,
} from "@/workers/finstack-contract";

/** Rust DecimalWire: the exact decimal text a monetary amount accepts. */
const decimalPattern = new RegExp(contracts.decimal.schema.pattern);
type Tab =
  | "statements"
  | "forecast"
  | "scenarios"
  | "ratios"
  | "valuation"
  | "monte-carlo"
  | "covenants"
  | "checks"
  | "advanced";
const tabs: { id: Tab; label: string }[] = [
  { id: "statements", label: "Statements & editing" },
  { id: "forecast", label: "Forecast & roll forward" },
  { id: "scenarios", label: "Scenarios" },
  { id: "ratios", label: "Metrics & ratios" },
  { id: "valuation", label: "Valuation" },
  { id: "monte-carlo", label: "Monte Carlo" },
  { id: "covenants", label: "Covenants" },
  { id: "checks", label: "Checks" },
  { id: "advanced", label: "Advanced" },
];
/** Rust EvalWarning, exactly as the native StatementResult reports it in meta.warnings. */
type EvalWarning = NonNullable<
  NonNullable<
    ReturnType<typeof useEvaluateStatement>["data"]
  >["meta"]["warnings"]
>[number];
/** One evaluation warning for display: its line (null when a capital-structure warning names none), period, Rust variant and remaining fields. */
interface WarningEntry {
  nodeId: string | null;
  period: string;
  kind: string;
  detail: string;
}
/** Flattens one externally tagged EvalWarning; a new Rust variant fails to type-check at capital_structure. */
function warningEntry(warning: EvalWarning): WarningEntry {
  if ("division_by_zero" in warning) {
    const { node_id, period } = warning.division_by_zero;
    return { nodeId: node_id, period, kind: "division_by_zero", detail: "" };
  }
  if ("non_finite_value" in warning) {
    const { node_id, period, value } = warning.non_finite_value;
    return {
      nodeId: node_id,
      period,
      kind: "non_finite_value",
      detail: String(value),
    };
  }
  if ("non_finite_skipped" in warning) {
    const {
      node_id,
      period,
      count,
      function: name,
    } = warning.non_finite_skipped;
    return {
      nodeId: node_id,
      period,
      kind: "non_finite_skipped",
      detail: `${count} dropped by ${name}`,
    };
  }
  const { period, warning: cause } = warning.capital_structure;
  const { kind, node_id, ...fields } = cause;
  return {
    nodeId: typeof node_id === "string" ? node_id : null,
    period,
    kind,
    detail: Object.entries(fields)
      .map(([name, value]) => `${name}: ${String(value)}`)
      .join(", "),
  };
}

/** Review supplied financials, edit explicit values and forecast inputs, and evaluate through native WASM. Wrap in FinstackQueryProvider. */
export function StatementsWorkbench({
  defaultModelJson,
  density = "compact",
  marketJson,
  asOf,
  defaultCheckSuiteJson,
  defaultScenarioSetJson,
  defaultRollupSpecJson,
  defaultCovenantEngineJson,
  selectedKey,
  onSelectedKeyChange,
  onModelAccepted,
  onRollForward,
}: {
  /** Complete canonical model supplied by the host or a financial-data service. Remount for a different document. */
  defaultModelJson: string;
  /** Row and text density of the workbench surface; compact by default. Sets SurfaceContext and data-density on the root, while data-theme is inherited from the host. */
  density?: "compact" | "comfortable";
  /** Both market JSON and ISO as-of must be supplied for market-aware evaluation. */
  marketJson?: string;
  asOf?: string;
  /** Optional canonical check suite supplied by the host. */
  defaultCheckSuiteJson?: string;
  /** Optional native ScenarioSet JSON that seeds the Scenarios tab. */
  defaultScenarioSetJson?: string;
  /** Optional native RollupSpec JSON whose methods seed the annual view; otherwise the methods the model declares do. */
  defaultRollupSpecJson?: string;
  /** Optional native CovenantEngine JSON that seeds the Covenants tab and is tested on open. */
  defaultCovenantEngineJson?: string;
  selectedKey?: string | null;
  onSelectedKeyChange?: (key: string | null) => void;
  onModelAccepted?: (canonicalJson: string) => void;
  /** Host obtains the next complete model from its financial-data service. The returned JSON is validated and evaluated natively. */
  onRollForward?: (
    canonicalJson: string,
    nextPeriodId: string,
  ) => Promise<string>;
}) {
  const { client, status, session } = useFinstack();
  const queryClient = useQueryClient();
  const validateModel = useStatementModelValidator();
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
  const [actionError, setActionError] = useState<string | null>(null);
  const [rolling, setRolling] = useState(false);
  // The model and result held before the last accepted roll forward.
  const [previousRoll, setPreviousRoll] = useState<EvaluatedModel | null>(null);
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
        const canonical = await validateModel(input);
        const next: StatementRequest = {
          modelJson: canonical,
          marketJson,
          asOf,
        };
        // Rejects a failing edit and caches the result under useEvaluateStatement's key (staleTime Infinity), so the hook does not evaluate again.
        await queryClient.query(
          evaluateStatementOptions(client, session, next),
        );
        if (token !== generation.current)
          return "A newer statement update superseded this one.";
        const ids = Object.keys(
          (JSON.parse(canonical) as FinancialModelSpecWire).nodes,
        );
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
        setRequest(next);
        // finstack-arith: remount revision counter, not a value
        setRevision((value) => value + 1);
        onModelAccepted?.(canonical);
        return null;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (token === generation.current) setActionError(message);
        return message;
      }
    },
    [
      client,
      status,
      session,
      queryClient,
      validateModel,
      marketPair,
      marketJson,
      asOf,
      onModelAccepted,
    ],
  );
  useEffect(() => {
    if (initialized.current || status !== "ready" || !client) return;
    initialized.current = true;
    void commit(defaultModelJson);
  }, [client, status, commit, defaultModelJson]);

  const evaluation = useEvaluateStatement(request);
  const evaluating =
    useIsFetching({ queryKey: ["finstack", session, "evaluateStatement"] }) > 0;
  const result = request?.modelJson === document ? evaluation.data : undefined;
  const resultJson = useMemo(
    () => (result ? serializeHost(result) : null),
    [result],
  );
  const warningCount = result?.meta.warnings?.length ?? 0;
  /** Native evaluation warnings grouped by line, in the order Rust reported them. */
  const warningGroups = useMemo(() => {
    const groups = new Map<string | null, WarningEntry[]>();
    for (const warning of result?.meta.warnings ?? []) {
      const entry = warningEntry(warning);
      groups.set(entry.nodeId, [...(groups.get(entry.nodeId) ?? []), entry]);
    }
    return [...groups];
  }, [result]);
  const [periodView, setPeriodView] = useState<"reported" | "annual">(
    "reported",
  );
  const [rollupMethods, setRollupMethods] = useState<Record<
    string,
    RollupMethod
  > | null>(null);
  const hostRollup = useMemo(() => {
    if (defaultRollupSpecJson === undefined) return null;
    try {
      return { methods: hostRollupMethods(defaultRollupSpecJson), error: null };
    } catch (error) {
      return {
        methods: null,
        error: `Default rollup spec: ${(error as Error).message}`,
      };
    }
  }, [defaultRollupSpecJson]);
  const defaultRollup = useDefaultRollupSpec(
    periodView === "annual" && request && !hostRollup
      ? { modelJson: request.modelJson, frequency: "annual" }
      : null,
  );
  /** The analyst's edits, else the host's spec, else the native default. Null until known. */
  const methods =
    rollupMethods ??
    hostRollup?.methods ??
    (defaultRollup.data ? { ...defaultRollup.data.methods } : null);
  const rollupRequest: StatementRollupRequest | null =
    periodView === "annual" && request && resultJson && methods
      ? {
          modelJson: request.modelJson,
          resultsJson: resultJson,
          specJson: annualRollupSpecJson(methods),
        }
      : null;
  const rollup = useRollupStatement(rollupRequest);
  const rollupError =
    hostRollup?.error ??
    defaultRollup.error?.message ??
    rollup.error?.message ??
    null;
  const annual = useMemo(
    () =>
      // A native rollup result always carries its annual period plan.
      model && rollup.data?.periods
        ? {
            model: { ...model, periods: rollup.data.periods },
            result: rollup.data,
          }
        : null,
    [model, rollup.data],
  );
  const reported = periodView === "reported";
  /** Columns and values the grids show: reported periods or the native annual rollup. */
  const view = reported
    ? model && result
      ? { model, result }
      : null
    : rollupRequest
      ? annual
      : null;
  const rows = useMemo(() => {
    if (!result || !model) return [];
    try {
      return statementRows(model, adaptStatementResult(result));
    } catch {
      return [];
    }
  }, [model, result]);
  /** The lines in display order: the source order once known, else the model's. */
  const nodeOptions = useMemo(
    () => (nodeIds.length ? nodeIds : Object.keys(model?.nodes ?? {})),
    [model, nodeIds],
  );
  const periodIds = useMemo(
    () => model?.periods.periods.map((period) => period.id) ?? [],
    [model],
  );
  const sections = useMemo((): StatementSections => {
    const tagged = (tag: string) =>
      nodeOptions.filter((id) => model?.nodes[id]?.tags?.includes(tag));
    const income = tagged("income_statement");
    const balance = nodeOptions.filter((id) =>
      model?.nodes[id]?.tags?.some(
        (tag) => tag === "balance_sheet" || tag === "debt",
      ),
    );
    const cash = tagged("cash_flow");
    const ratios = nodeOptions.filter((id) =>
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
      other: nodeOptions.filter((id) => !shown.has(id)),
    };
  }, [model, nodeOptions]);
  const selected = useMemo(() => {
    if (!model || !link.selectedKey) return null;
    for (const nodeId of Object.keys(model.nodes))
      for (const period of model.periods.periods)
        if (statementCellKey(nodeId, period.id) === link.selectedKey)
          return { nodeId, periodId: period.id };
    return null;
  }, [model, link.selectedKey]);
  const activeNode =
    selected?.nodeId ?? sections.income[0] ?? nodeIds[0] ?? null;
  const activePeriod =
    selected?.periodId ??
    editPeriod ??
    model?.periods.periods.findLast((period) => period.is_actual)?.id ??
    model?.periods.periods[0]?.id ??
    null;
  const headline = rows.filter((row) =>
    model?.nodes[row.nodeId]?.tags?.some(
      (tag) => tag === "headline" || tag === "covenant",
    ),
  );
  const entity =
    typeof model?.meta?.entity === "string"
      ? model.meta.entity
      : (model?.id ?? "Financial statements");
  const currency =
    typeof model?.meta?.currency === "string" ? model.meta.currency : null;
  const nextForecast = model?.periods.periods.find(
    (period) => !period.is_actual,
  );
  const rollForward = async () => {
    if (!onRollForward || !request || !nextForecast) return;
    const before =
      resultJson === null ? null : { modelJson: request.modelJson, resultJson };
    const token = ++generation.current;
    setRolling(true);
    setActionError(null);
    try {
      const updated = await onRollForward(request.modelJson, nextForecast.id);
      const rolled = financialModelModule.codec.parse(
        updated,
      ) as FinancialModelSpecWire;
      if (
        !rolled.periods.periods.some(
          (period) => period.id === nextForecast.id && period.is_actual,
        )
      )
        throw new Error(
          `Updated financials must mark ${nextForecast.id} actual.`,
        );
      if (token === generation.current && (await commit(updated)) === null)
        setPreviousRoll(before);
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
      if (!decimalPattern.test(value.trim()))
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
  return (
    <SurfaceContext.Provider value={{ density }}>
      <section
        aria-label="Statements workbench"
        data-density={density}
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
                {model?.periods.periods.length
                  ? ` · ${model.periods.periods.length} reporting periods`
                  : ""}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <span
                className="rounded-full border border-border px-3 py-1 text-xs"
                role="status"
              >
                {evaluating
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
            <div
              role="group"
              aria-label="Reporting periods"
              className="flex flex-wrap gap-2"
            >
              {model.periods.periods.map((period) => (
                <Button
                  key={period.id}
                  type="button"
                  variant="outline"
                  size="sm"
                  aria-pressed={activePeriod === period.id}
                  onClick={() => choosePeriod(period.id)}
                >
                  <span className="font-mono">{period.id}</span>{" "}
                  <span className="ml-1 text-muted-foreground">
                    {period.is_actual ? "Historical" : "Forecast"}
                  </span>
                </Button>
              ))}
            </div>
          )}
          {warningCount > 0 && (
            <details>
              <summary>Evaluation warnings ({warningCount})</summary>
              <ul
                aria-label="Evaluation warnings"
                className="mt-2 space-y-2 text-sm"
              >
                {warningGroups.map(([nodeId, entries]) => {
                  const label =
                    nodeId === null
                      ? "Capital structure"
                      : (model?.nodes[nodeId]?.name ?? nodeId);
                  return (
                    <li key={nodeId ?? ""}>
                      <span className="font-medium">{label}</span>
                      {nodeId !== null && (
                        <span className="ml-2 font-mono text-xs text-muted-foreground">
                          {nodeId}
                        </span>
                      )}
                      <ul
                        aria-label={`${label} warnings`}
                        className="mt-1 space-y-0.5 pl-4 text-xs text-muted-foreground"
                      >
                        {entries.map((entry, index) => (
                          <li key={index}>
                            <span className="font-mono">{entry.period}</span>
                            {" · "}
                            <code>{entry.kind}</code>
                            {entry.detail && ` · ${entry.detail}`}
                          </li>
                        ))}
                      </ul>
                    </li>
                  );
                })}
              </ul>
            </details>
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
            <div
              className="rounded-lg border border-border bg-card p-4"
              key={row.nodeId}
            >
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
        <div className="flex flex-wrap items-center justify-end gap-2 text-xs">
          <span className="text-muted-foreground">Periods</span>
          <div role="group" aria-label="Periods" className="flex gap-1">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-pressed={reported}
              onClick={() => setPeriodView("reported")}
            >
              Reported
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-pressed={!reported}
              onClick={() => setPeriodView("annual")}
            >
              Annual
            </Button>
          </div>
          <span className="ml-3 text-muted-foreground">Display units</span>
          <div role="group" aria-label="Display units" className="flex gap-1">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-pressed={displayMode === "millions"}
              onClick={() => setDisplayMode("millions")}
            >
              Millions
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-pressed={displayMode === "exact"}
              onClick={() => setDisplayMode("exact")}
            >
              Exact
            </Button>
          </div>
        </div>
        <Tabs value={tab} onValueChange={(value) => setTab(value as Tab)}>
          <TabsList
            variant="line"
            aria-label="Statement workspace"
            className="finstack-workbench__tabs"
          >
            {tabs.map((item) => (
              <TabsTrigger key={item.id} value={item.id}>
                {item.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {!model && <p role="alert">The supplied model is not valid JSON.</p>}
          {!result && !evaluating && !actionError && (
            <p className="text-sm text-muted-foreground" role="status">
              Loading supplied financial statements…
            </p>
          )}
          <TabsContent value="statements">
            {model && request && resultJson && (
              <StatementsPanel
                model={model}
                modelJson={request.modelJson}
                resultJson={resultJson}
                nodeOptions={nodeOptions}
                sections={sections}
                view={view}
                reported={reported}
                methods={methods}
                onMethodsChange={setRollupMethods}
                rollupError={rollupError}
                link={link}
                displayMode={displayMode}
                activeNode={activeNode}
                activePeriod={activePeriod}
                onEditValue={saveCell}
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="forecast">
            {model && (
              <ForecastPanel
                model={model}
                revision={revision}
                onCommit={(json) => commit(json, true)}
                nextForecastId={nextForecast?.id}
                onRollForward={
                  onRollForward ? () => void rollForward() : undefined
                }
                rolling={rolling}
                ready={status === "ready"}
                onLoadUpdated={() => setTab("advanced")}
                previousRoll={previousRoll}
                current={
                  request && resultJson
                    ? { modelJson: request.modelJson, resultJson }
                    : null
                }
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="scenarios">
            {model && request && (
              <ScenarioPanel
                model={model}
                modelJson={request.modelJson}
                nodeIds={nodeOptions}
                marketJson={marketJson}
                asOf={asOf}
                activePeriod={activePeriod}
                displayMode={displayMode}
                link={link}
                defaultScenarioSetJson={defaultScenarioSetJson}
                density={density}
              />
            )}
            {model && request && result && (
              <div className="mt-8 border-t border-border pt-6">
                <SensitivityPanel
                  modelJson={request.modelJson}
                  result={result}
                  nodeIds={nodeOptions}
                  periods={periodIds}
                  onApply={(json) => commit(json, true)}
                  density={density}
                />
              </div>
            )}
          </TabsContent>
          <TabsContent value="ratios">
            {model && request && resultJson && (
              <RatiosPanel
                model={model}
                modelJson={request.modelJson}
                resultJson={resultJson}
                nodeOptions={nodeOptions}
                periodIds={periodIds}
                sections={sections}
                view={view}
                reported={reported}
                rollupError={rollupError}
                link={link}
                displayMode={displayMode}
                activePeriod={activePeriod}
                currency={currency}
                onCommit={(json) => commit(json, true)}
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="valuation">
            {model && request && (
              <ValuationPanel
                modelJson={request.modelJson}
                nodeIds={nodeOptions}
                marketJson={marketJson}
                asOf={asOf}
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="monte-carlo">
            {model && request && (
              <MonteCarloPanel
                modelJson={request.modelJson}
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="covenants">
            {model && result && resultJson && (
              <CovenantsPanel
                resultsJson={resultJson}
                nodeIds={nodeOptions}
                defaultEngineJson={defaultCovenantEngineJson}
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="checks">
            {model && result && resultJson && (
              <div className="max-w-5xl">
                <StatementChecks
                  key={revision}
                  modelJson={request!.modelJson}
                  resultsJson={resultJson}
                  defaultConfigJson={defaultCheckSuiteJson}
                  link={link}
                  density={density}
                />
              </div>
            )}
          </TabsContent>
          <TabsContent value="advanced">
            {model && (
              <AdvancedPanel
                document={document}
                resultJson={resultJson}
                revision={revision}
                importText={importText}
                onImportTextChange={setImportText}
                ready={status === "ready"}
                onLoad={() => void commit(importText)}
                onCommit={(json) => commit(json, true)}
                density={density}
              />
            )}
          </TabsContent>
        </Tabs>
      </section>
    </SurfaceContext.Provider>
  );
}
