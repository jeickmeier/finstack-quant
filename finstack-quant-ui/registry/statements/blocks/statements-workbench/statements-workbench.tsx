"use client";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useIsFetching, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { ForecastEditor } from "./forecast-editor";
import { MetricEditor } from "./metric-editor";
import { ScenarioPanel } from "./scenario-panel";
import {
  annualRollupSpecJson,
  initialRollupMethods,
  rolledPeriods,
  RollupMethodsEditor,
  type RollupMethod,
} from "./rollup";
import { FinancialModelEditor } from "@/components/finstack/statements/components/financial-model-editor/financial-model-editor";
import { financialModelModule } from "@/components/finstack/statements/components/financial-model-editor/model";
import { StatementGrid } from "@/components/finstack/statements/components/statement-grid/statement-grid";
import { StatementChart } from "@/components/finstack/statements/components/statement-chart/statement-chart";
import { StatementExplanation } from "@/components/finstack/statements/components/statement-diagnostics/statement-explanation";
import { StatementChecks } from "@/components/finstack/statements/components/statement-diagnostics/statement-checks";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { SurfaceContext } from "@/components/finstack/shared/primitives/surface/surface";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import {
  evaluateStatementOptions,
  useEvaluateStatement,
} from "@/hooks/statements/use-evaluate-statement/use-evaluate-statement";
import { useStatementModelValidator } from "@/hooks/statements/use-statement-model-validator/use-statement-model-validator";
import { useRollupStatement } from "@/hooks/statements/use-rollup-statement/use-rollup-statement";
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
  "statements" | "forecast" | "scenarios" | "ratios" | "checks" | "advanced";
const tabs: { id: Tab; label: string }[] = [
  { id: "statements", label: "Statements & editing" },
  { id: "forecast", label: "Forecast & roll forward" },
  { id: "scenarios", label: "Scenarios" },
  { id: "ratios", label: "Metrics & ratios" },
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
  /** Optional native RollupSpec JSON whose methods seed the annual view; otherwise methods follow line tags. */
  defaultRollupSpecJson?: string;
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
  const methods = useMemo(() => {
    if (rollupMethods) return rollupMethods;
    if (!model) return {};
    try {
      return initialRollupMethods(model, defaultRollupSpecJson);
    } catch {
      return {};
    }
  }, [rollupMethods, model, defaultRollupSpecJson]);
  const rollupRequest: StatementRollupRequest | null =
    periodView === "annual" && request && resultJson
      ? {
          modelJson: request.modelJson,
          resultsJson: resultJson,
          specJson: annualRollupSpecJson(methods),
        }
      : null;
  const rollup = useRollupStatement(rollupRequest);
  const annual = useMemo(
    () =>
      model && rollup.data
        ? {
            model: { ...model, periods: rolledPeriods(model, rollup.data) },
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
              {model.periods.map((period) => (
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
            {model && result && resultJson && (
              <div className="space-y-6">
                <p className="text-xs text-muted-foreground">
                  {reported
                    ? "Select a period above to review it. Click a reported or forecast value to edit in exact units; calculated lines update from their drivers."
                    : "Annual values are rolled up natively from the reported periods. Switch to Reported to edit values."}
                </p>
                {!reported && (
                  <RollupMethodsEditor
                    model={model}
                    nodeIds={
                      nodeIds.length ? nodeIds : Object.keys(model.nodes)
                    }
                    methods={methods}
                    onChange={setRollupMethods}
                  />
                )}
                {!reported && rollup.error && (
                  <p
                    role="alert"
                    className="rounded-md border border-error p-3 text-sm text-error"
                  >
                    {rollup.error.message}
                  </p>
                )}
                {!view && !rollup.error && (
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
                            onEditValue={reported ? saveCell : undefined}
                            density={density}
                          />
                        ),
                    )}
                  </div>
                )}
                {reported && activeNode && activePeriod && (
                  <details>
                    <summary>Explain selected value</summary>
                    <div className="mt-3">
                      <StatementExplanation
                        request={{
                          modelJson: request!.modelJson,
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
            )}
          </TabsContent>
          <TabsContent value="forecast">
            {model && (
              <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_20rem]">
                <div className="space-y-4">
                  <div>
                    <h3 className="text-lg font-semibold">
                      Forecast assumptions
                    </h3>
                    <p className="text-sm text-muted-foreground">
                      Choose a forecast method and edit its assumptions. Rates
                      are decimals; levels use exact model units. WASM
                      calculates the projected values.
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
                        return commit(JSON.stringify(next), true);
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
                        Start with carry forward, then choose the projection
                        method and assumptions above.
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
                  {nextForecast ? (
                    <>
                      <p className="text-sm">
                        Next forecast period:{" "}
                        <strong className="font-mono">{nextForecast.id}</strong>
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Bring in the next complete model with reported values
                        and updated forecast assumptions. The workbench
                        validates it and refreshes every statement and ratio.
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
                      All supplied periods are actual. Load a model with
                      forecast periods to continue.
                    </p>
                  )}
                  <Button
                    type="button"
                    variant="link"
                    size="sm"
                    onClick={() => setTab("advanced")}
                  >
                    Load updated financials
                  </Button>
                </aside>
              </div>
            )}
          </TabsContent>
          <TabsContent value="scenarios">
            {model && request && (
              <ScenarioPanel
                model={model}
                modelJson={request.modelJson}
                nodeIds={nodeIds.length ? nodeIds : Object.keys(model.nodes)}
                marketJson={marketJson}
                asOf={asOf}
                activePeriod={activePeriod}
                displayMode={displayMode}
                link={link}
                defaultScenarioSetJson={defaultScenarioSetJson}
                density={density}
              />
            )}
          </TabsContent>
          <TabsContent value="ratios">
            {model && result && (
              <div className="space-y-5">
                <MetricEditor
                  model={model}
                  onApply={(json) => commit(json, true)}
                />
                <p className="text-sm text-muted-foreground">
                  Every value below is calculated from the current financials by
                  WASM.
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
                      <p
                        className="text-sm text-muted-foreground"
                        role="status"
                      >
                        {rollup.error?.message ??
                          "Rolling up to annual periods…"}
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
              </div>
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
              <div className="space-y-6">
                <div className="rounded-lg border border-border bg-card p-4">
                  <h3 className="font-semibold">Load updated financials</h3>
                  <p className="mt-1 text-sm text-muted-foreground">
                    Paste the complete canonical FinancialModelSpec JSON
                    returned by your data provider or parsing service.
                  </p>
                  <div className="mt-3">
                    <FieldFrame label="Model JSON">
                      {(control) => (
                        <Textarea
                          {...control}
                          className="min-h-36 font-mono"
                          value={importText}
                          onChange={(event) =>
                            setImportText(event.target.value)
                          }
                          spellCheck={false}
                        />
                      )}
                    </FieldFrame>
                  </div>
                  <Button
                    type="button"
                    className="mt-3"
                    disabled={status !== "ready"}
                    onClick={() => void commit(importText)}
                  >
                    Load & evaluate
                  </Button>
                </div>
                <details>
                  <summary>Model structure and formulas</summary>
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
                <details>
                  <summary>Canonical model and native result</summary>
                  <div className="mt-3 space-y-4">
                    <JsonViewer
                      label="Canonical model JSON"
                      text={document}
                      downloadName="financial-model.json"
                      density={density}
                    />
                    {resultJson && (
                      <JsonViewer
                        label="Native statement result"
                        text={resultJson}
                        downloadName="statement-result.json"
                        density={density}
                      />
                    )}
                  </div>
                </details>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </section>
    </SurfaceContext.Provider>
  );
}
