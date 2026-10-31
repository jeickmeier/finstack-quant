"use client";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { InstrumentForm } from "@/components/finstack/valuations/components/instrument-form/instrument-form";
import { MarketContextForm } from "@/components/finstack/core/components/market-context-form/market-context-form";
import { CashflowViewer } from "@/components/finstack/valuations/components/cashflow-viewer/cashflow-viewer";
import { ExplanationTrace } from "@/components/finstack/valuations/components/explanation-trace/explanation-trace";
import { ValuationDetails } from "@/components/finstack/valuations/components/valuation-details/valuation-details";
import {
  MarketContextBrowser,
  type MarketContextBrowserProps,
} from "@/components/finstack/core/components/market-context-browser/market-context-browser";
import {
  VolCubeExplorer,
  type VolCubeExplorerProps,
} from "@/components/finstack/models/components/vol-cube-explorer/vol-cube-explorer";
import { VolSurfaceChart } from "@/components/finstack/core/components/vol-surface-chart/vol-surface-chart";
import { SurfaceArbitrageBadge } from "@/components/finstack/models/components/surface-arbitrage-badge/surface-arbitrage-badge";
import {
  FxSurfaceChart,
  type FxSurfaceChartProps,
} from "@/components/finstack/models/components/fx-surface-chart/fx-surface-chart";
import { DealPresetPanel } from "./deal-preset-panel";
import { StructuredCreditPanel } from "./structured-credit-panel";
import { CalibrationPanel } from "./calibration-panel";
import {
  MarketScenarioInputs,
  MarketScenarioResults,
  noComparison,
  useMarketScenarioSession,
} from "./market-scenario-panel";
import { AttributionResults, useAttributionSession } from "./attribution-panel";
import type { CalibrationFitChartProps } from "@/components/finstack/calibration/components/calibration-fit-chart/calibration-fit-chart";
import { MeasuresGrid } from "@/components/finstack/valuations/components/measures-grid/measures-grid";
import {
  DimensionalMeasures,
  dimensionalMeasureCount,
} from "@/components/finstack/valuations/components/dimensional-measures/dimensional-measures";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { SurfaceContext } from "@/components/finstack/shared/primitives/surface/surface";
import type { PriceRequest } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { resolveModel } from "@/workers/finstack-contract";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { useWorkbenchSession } from "./use-workbench-session";
import { WorkbenchSettings } from "./workbench-settings";
export function PricingWorkbench({
  defaultRequest,
  density = "compact",
  defaultInstrumentType,
  defaultResultTab = "cashflows",
  exampleRequests,
  exampleResultTabs,
  onInstrumentTypeChange,
  defaultCalibrationJson,
  surfaceOptions,
  cubeOptions,
  fxOptions,
  calibrationChartOptions,
  scenario,
  marketScenario,
  brand,
}: {
  /** Complete initial instrument request and canonical market snapshot. Remount to load another document. */
  defaultRequest: PriceRequest;
  density?: "compact" | "comfortable";
  /** Host-owned deep link selects this canonical example; supplied market/parameters stay explicit. */
  defaultInstrumentType?: string;
  /** Initial secondary result view for the supplied example. */
  defaultResultTab?: "cashflows" | "measures" | "buckets" | "diagnostics";
  /** Complete host-owned examples. Selecting one replaces its instrument, market and pricing context together. */
  exampleRequests?: Readonly<Record<string, PriceRequest>>;
  /** Result view to open when selecting a prepared example. */
  exampleResultTabs?: Readonly<
    Record<string, "cashflows" | "measures" | "buckets" | "diagnostics">
  >;
  /** Notify the host when a prepared instrument example is selected. */
  onInstrumentTypeChange?: (type: string) => void;
  defaultCalibrationJson?: string;
  /** Optional host seeds for the structured-credit tab, which is offered while structured credit is selected and evaluated on its completed request only. */
  scenario?: {
    /** Initially selected tranche; omitted means the first tranche the deal returns. */
    trancheId?: string;
    /** Initial native ScenarioGrid JSON; omitted means the schema's Rust example. */
    gridJson?: string;
    /** Display extent of the price heatmap; omitted means the returned prices about par. */
    priceDomain?: readonly [number, number];
    /** Registry metric ids valued on the tranche by trancheMetrics; omitted means none. */
    trancheMetrics?: readonly string[];
  };
  /** Initial ScenarioSpec JSON for the "Market scenario" tab; it is applied only on request. Defaults to an empty scenario. */
  marketScenario?: string;
  /** Stored-surface view options. A returned `forwards` key adds the native static-arbitrage badge (null or empty: forwards not supplied); forwards are host-supplied, never inferred. */
  surfaceOptions?(surface: MarketContextStateWire["surfaces"][number]):
    | (NonNullable<
        ReturnType<NonNullable<MarketContextBrowserProps["surfaceOptions"]>>
      > & {
        /** Forward prices in the surface's strike units: one to broadcast or one per expiry. */
        forwards?: readonly number[] | null;
      })
    | undefined;
  cubeOptions?(
    cube: MarketContextStateWire["vol_cubes"][number],
  ): Omit<VolCubeExplorerProps, "cube"> | undefined;
  fxOptions?(
    surface: MarketContextStateWire["fx_delta_vol_surfaces"][number],
  ): Omit<FxSurfaceChartProps, "surface"> | undefined;
  calibrationChartOptions?: Omit<
    CalibrationFitChartProps,
    "stepId" | "report" | "marketData" | "link"
  >;
  /** Wordmark or logo for the workbench bar; the registry ships none. */
  brand?: ReactNode;
}) {
  const {
    initial,
    initialType,
    type,
    instrumentRevision,
    scenariosOpen,
    setScenariosOpen,
    resultTab,
    setResultTab,
    currentDensity,
    setCurrentDensity,
    theme,
    setTheme,
    instrumentOpen,
    setInstrumentOpen,
    activeTab,
    setActiveTab,
    marketTab,
    setMarketTab,
    marketDocument,
    setMarketDocument,
    market,
    acceptMarket,
    setInstrument,
    builtInstrumentJson,
    loadInstrument,
    selectType,
    printSnapshot,
    startPrint,
    printing,
    shown,
    measureMetadata,
    cashflows,
    cashflowLadder,
    cashflowModel,
    preparing,
    printMarket,
    worker,
    validate,
    validateMarket,
    models,
    metrics,
    candidate,
    price,
    request,
    setRequest,
    params,
    setParams,
    stale,
    status,
    stateKind,
    currentError,
    meta,
    rounding,
    measureGridProps,
    pricingContext,
    completed,
    priceDriverFields,
  } = useWorkbenchSession({
    defaultRequest,
    density,
    defaultInstrumentType,
    defaultResultTab,
    exampleRequests,
    exampleResultTabs,
    onInstrumentTypeChange,
  });
  const marketScenarioSession = useMarketScenarioSession(
    marketScenario,
    completed?.request ?? null,
  );
  const attributionSession = useAttributionSession(
    completed?.request ?? null,
    marketScenarioSession,
  );
  // The shocked valuation fills the existing comparison columns; no difference is computed.
  const comparison = printSnapshot
    ? noComparison
    : marketScenarioSession.comparison;
  const dimensionalCount = dimensionalMeasureCount({
    result: shown?.result,
    metadata: measureMetadata.data,
  });
  return (
    <SurfaceContext.Provider
      value={{
        theme: printSnapshot ? "light" : theme,
        density: currentDensity,
      }}
    >
      <div
        className="finstack-workbench-container"
        data-theme={printSnapshot ? "light" : theme}
        data-density={currentDensity}
      >
        <Tabs
          value={printSnapshot ? "market" : activeTab}
          onValueChange={setActiveTab}
        >
          <section
            aria-label="Pricing workbench"
            onKeyDown={(event) => {
              const target = event.target as Element;
              if (
                event.altKey ||
                event.ctrlKey ||
                event.metaKey ||
                target.closest("[data-finstack-print]") !==
                  event.currentTarget ||
                target.closest(
                  "input,textarea,select,[contenteditable=true],[role=combobox],[role=listbox],[role=menu],[role=dialog]",
                )
              )
                return;
              if (event.key === "1" || event.key === "2") {
                event.preventDefault();
                setActiveTab(event.key === "1" ? "instrument" : "market");
              }
              if (["3", "4", "5"].includes(event.key)) {
                event.preventDefault();
                setResultTab(
                  { "3": "cashflows", "4": "diagnostics", "5": "trace" }[
                    event.key
                  ]!,
                );
              }
            }}
            data-finstack-print=""
            data-stale={stale && !printSnapshot ? "" : undefined}
            data-printing={
              printSnapshot ? (preparing ? "preparing" : "ready") : undefined
            }
            data-theme={printSnapshot ? "light" : theme}
            data-density={currentDensity}
            className="finstack-workbench font-sans text-sm text-foreground focus-visible:outline-2 focus-visible:outline-ring"
          >
            <header className="finstack-workbench__bar print:hidden">
              {brand != null && (
                <h2 className="finstack-workbench__brand">{brand}</h2>
              )}
              <span className="finstack-workbench__id">
                {shown?.result.instrument_id ?? type}
              </span>
              <TabsList
                variant="line"
                aria-label="Pricing inputs"
                className="finstack-workbench__tabs"
              >
                <TabsTrigger value="instrument">
                  <span className="finstack-kbd">1</span> Instrument
                </TabsTrigger>
                <TabsTrigger value="market">
                  <span className="finstack-kbd">2</span> Market
                </TabsTrigger>
                <TabsTrigger value="calibrate">Calibrate</TabsTrigger>
                <TabsTrigger value="market-scenario">
                  Market scenario
                </TabsTrigger>
              </TabsList>
              <div className="finstack-workbench__context">
                {pricingContext}
              </div>
              <Button
                size="sm"
                type="button"
                disabled={!candidate || price.isFetching}
                onClick={() => {
                  if (request === candidate) void price.refetch();
                  else setRequest(candidate);
                }}
              >
                Price
              </Button>
              <WorkbenchSettings
                params={params}
                onParamsChange={setParams}
                instrumentType={type}
                models={models.data ?? {}}
                metrics={metrics.data ?? {}}
                loading={models.isPending || metrics.isPending}
                error={models.error?.message ?? metrics.error?.message}
                density={currentDensity}
                onDensityChange={setCurrentDensity}
                theme={theme}
                onThemeChange={setTheme}
              />
              <Button
                variant="outline"
                size="sm"
                type="button"

                disabled={!completed || status !== "Priced" || printing}
                onClick={startPrint}
              >
                {printing ? "Preparing report…" : "Print report"}
              </Button>{" "}
              <p
                role="status"
                aria-live="polite"
                className="finstack-workbench__state"
                data-state={stateKind}
              >
                {status}
              </p>
            </header>
            <section aria-label="Inputs" className="finstack-workbench__inputs">
              {worker.error && (
                <p role="alert" className="text-error">
                  {worker.error.message}
                </p>
              )}
              {printSnapshot && shown && (
                <JsonViewer
                  label="Priced instrument JSON"
                  text={shown.request.instrumentJson}
                  density={currentDensity}
                />
              )}
              <TabsContent
                value="instrument"
                keepMounted
                className="finstack-workbench__panel print:hidden"
              >
                {type === "structured_credit" && (
                  <DealPresetPanel onDeal={loadInstrument} />
                )}
                <InstrumentForm
                  key={instrumentRevision}
                  type={type}
                  onTypeChange={selectType}
                  allowedTypes={
                    exampleRequests ? Object.keys(exampleRequests) : undefined
                  }
                  showGeneratedExample={!exampleRequests}
                  priceDriverFields={priceDriverFields}
                  defaultJson={
                    builtInstrumentJson ??
                    exampleRequests?.[type]?.instrumentJson ??
                    ((defaultInstrumentType ?? initialType) === initialType
                      ? initial.instrumentJson
                      : undefined)
                  }
                  validate={validate}
                  onValidated={setInstrument}
                  onSubmit={setInstrument}
                  submitVariant="outline"
                />
              </TabsContent>
              <TabsContent
                value="market"
                keepMounted
                className="finstack-workbench__panel"
              >
                <Tabs
                  value={printSnapshot ? "view" : marketTab}
                  onValueChange={setMarketTab}
                >
                  <TabsList
                    variant="line"
                    aria-label="Market mode"
                    className="finstack-workbench__tabs mb-2 print:hidden"
                  >
                    <TabsTrigger value="view">View market</TabsTrigger>
                    <TabsTrigger value="edit">Edit market</TabsTrigger>
                    <TabsTrigger value="json">Snapshot JSON</TabsTrigger>
                  </TabsList>
                  <TabsContent
                    value="edit"
                    keepMounted
                    className="print:hidden"
                  >
                    <MarketContextForm
                      key={marketDocument.revision}
                      defaultJson={marketDocument.json}
                      validate={validateMarket}
                      onValidated={acceptMarket}
                      onSubmit={acceptMarket}
                    />
                  </TabsContent>
                  <TabsContent value="json" className="print:hidden">
                    <JsonViewer
                      label="Market snapshot"
                      text={market?.json ?? initial.marketJson}
                      density={currentDensity}
                    />
                  </TabsContent>
                  <TabsContent value="view" keepMounted>
                    {printSnapshot && (
                      <JsonViewer
                        label="Market snapshot"
                        text={printSnapshot.request.marketJson}
                        density={currentDensity}
                      />
                    )}
                    {market ? (
                      <MarketContextBrowser
                        state={printMarket ?? market.state}
                        surfaceOptions={surfaceOptions}
                        renderObject={(entry) => {
                          const [field, index] = entry.path;
                          if (typeof index !== "number") return undefined;
                          if (field === "vol_cubes") {
                            const cube = (printMarket ?? market.state)
                                .vol_cubes[index]!,
                              options = cubeOptions?.(cube);
                            return options ? (
                              <VolCubeExplorer
                                key={cube.id}
                                {...options}
                                cube={cube}
                              />
                            ) : undefined;
                          }
                          if (field === "surfaces") {
                            const surface = (printMarket ?? market.state)
                                .surfaces[index]!,
                              options = surfaceOptions?.(surface);
                            if (!options || !("forwards" in options))
                              return undefined;
                            const { forwards, ...chart } = options;
                            return (
                              <div className="space-y-2">
                                <SurfaceArbitrageBadge
                                  key={surface.id}
                                  surface={surface}
                                  forwards={forwards}
                                  density={currentDensity}
                                />
                                <VolSurfaceChart surface={surface} {...chart} />
                              </div>
                            );
                          }
                          if (field === "fx_delta_vol_surfaces") {
                            const surface = (printMarket ?? market.state)
                                .fx_delta_vol_surfaces[index]!,
                              options = fxOptions?.(surface);
                            return options ? (
                              <FxSurfaceChart {...options} surface={surface} />
                            ) : undefined;
                          }
                          return undefined;
                        }}
                      />
                    ) : (
                      <p role="status">Preparing validated market…</p>
                    )}
                  </TabsContent>
                </Tabs>
              </TabsContent>
              <TabsContent
                value="calibrate"
                keepMounted
                className="finstack-workbench__panel print:hidden"
              >
                <CalibrationPanel
                  defaultJson={defaultCalibrationJson}
                  chartOptions={calibrationChartOptions}
                  onMarket={(json) => {
                    acceptMarket(json);
                    setMarketDocument((current) => ({
                      json,
                      // finstack-arith: remount revision counter, not a value
                      revision: current.revision + 1,
                    }));
                  }}
                />
              </TabsContent>
              <TabsContent
                value="market-scenario"
                className="finstack-workbench__panel print:hidden"
              >
                <MarketScenarioInputs
                  session={marketScenarioSession}
                  curveIds={market?.state.curves.map((curve) => curve.id) ?? []}
                  priced={completed !== null}
                  density={currentDensity}
                />
              </TabsContent>
            </section>
            <section
              aria-label="Results"
              className="finstack-workbench__results"
            >
              <div className="finstack-workbench__summary">
                <header className="finstack-workbench__results-header">
                  <h2 className="finstack-title">Results</h2>
                  {stale && !printSnapshot ? (
                    <span className="finstack-workbench__stale">
                      Stale · reprice to update
                    </span>
                  ) : (
                    <span className="text-xs text-muted-foreground">
                      {shown
                        ? `${Object.keys(shown.result.measures).length} metrics · grouped`
                        : "Awaiting valuation"}
                    </span>
                  )}
                </header>
                <div className="finstack-workbench__summary-body">
                  <MeasuresGrid
                    {...measureGridProps}
                    {...comparison}
                    content={printSnapshot ? "all" : "summary"}
                  />
                  {printSnapshot && shown && (
                    <div className="hidden print:block">
                      <ValuationDetails
                        result={shown.result}
                        density={currentDensity}
                      />
                    </div>
                  )}
                </div>
              </div>
              <Tabs
                value={printSnapshot ? "cashflows" : resultTab}
                onValueChange={setResultTab}
                className="finstack-workbench__detail"
              >
                <div className="finstack-workbench__results-header print:hidden">
                  <TabsList
                    variant="line"
                    aria-label="Result views"
                    className="max-w-full flex-wrap"
                    style={{ height: "auto", minHeight: "2rem" }}
                  >
                    <TabsTrigger value="measures">Measures</TabsTrigger>
                    {(dimensionalCount > 0 || resultTab === "buckets") && (
                      <TabsTrigger value="buckets">
                        Buckets &amp; surfaces
                        {dimensionalCount > 0 ? ` · ${dimensionalCount}` : ""}
                      </TabsTrigger>
                    )}
                    <TabsTrigger value="cashflows">
                      <span className="finstack-kbd">3</span> Cashflows
                    </TabsTrigger>
                    <TabsTrigger value="diagnostics">
                      <span className="finstack-kbd">4</span> Diagnostics
                    </TabsTrigger>
                    <TabsTrigger value="trace">
                      <span className="finstack-kbd">5</span> Trace
                    </TabsTrigger>
                    <TabsTrigger value="market-scenario">
                      Market scenario
                    </TabsTrigger>
                    <TabsTrigger value="pnl-explain">
                      P&amp;L explain
                    </TabsTrigger>
                    <TabsTrigger value="request">Request</TabsTrigger>
                    {type === "structured_credit" && (
                      <TabsTrigger value="scenarios">Scenarios</TabsTrigger>
                    )}
                  </TabsList>
                </div>
                <TabsContent value="measures" className="print:hidden">
                  <MeasuresGrid
                    {...measureGridProps}
                    {...comparison}
                    content="measures"
                    excludeDimensional
                  />
                </TabsContent>
                <TabsContent value="buckets" className="print:hidden">
                  {shown &&
                  !measureMetadata.data &&
                  measureMetadata.isPending ? (
                    <p
                      role="status"
                      className="px-1 py-4 text-sm text-muted-foreground"
                    >
                      Loading measure coordinates…
                    </p>
                  ) : (
                    <DimensionalMeasures
                      result={shown?.result}
                      metadata={measureMetadata.data}
                      compareTo={comparison.compareTo}
                      comparisonMetadata={comparison.comparisonMetadata}
                    />
                  )}
                </TabsContent>
                <TabsContent value="cashflows" keepMounted>
                  {shown && !cashflowModel ? (
                    <p role="status" className="text-muted-foreground">
                      Cashflows are exported only for the discounting and
                      hazard_rate models.
                    </p>
                  ) : shown ? (
                    <CashflowViewer
                      density={currentDensity}
                      cashflows={cashflows.data}
                      ladder={cashflowLadder.data ?? null}
                      ladderError={cashflowLadder.error?.message}
                      loading={
                        cashflows.isFetching ||
                        cashflows.workerStatus === "starting"
                      }
                      error={cashflows.error?.message}
                    />
                  ) : (
                    <p role="status" className="text-muted-foreground">
                      Cashflows will appear after a valid valuation.
                    </p>
                  )}
                </TabsContent>
                <TabsContent value="diagnostics" className="print:hidden">
                  {shown ? (
                    <ValuationDetails
                      result={shown.result}
                      density={currentDensity}
                      includeExplanation={false}
                    />
                  ) : (
                    <p>No valuation details returned</p>
                  )}
                </TabsContent>
                <TabsContent value="trace" className="print:hidden">
                  {shown?.result.explanation != null ? (
                    <ExplanationTrace
                      value={shown.result.explanation}
                      density={currentDensity}
                    />
                  ) : (
                    <p className="text-muted-foreground">
                      No explanation trace returned
                    </p>
                  )}
                </TabsContent>
                <TabsContent value="market-scenario" className="print:hidden">
                  <MarketScenarioResults
                    session={marketScenarioSession}
                    density={currentDensity}
                  />
                </TabsContent>
                <TabsContent value="pnl-explain" className="print:hidden">
                  <AttributionResults
                    session={attributionSession}
                    density={currentDensity}
                  />
                </TabsContent>
                <TabsContent value="request" className="print:hidden">
                  <details
                    open={instrumentOpen}
                    onToggle={(event) =>
                      setInstrumentOpen(event.currentTarget.open)
                    }
                  >
                    <summary>Priced instrument</summary>
                    <JsonViewer
                      label="Priced instrument JSON"
                      text={shown?.request.instrumentJson ?? null}
                      density={currentDensity}
                    />
                  </details>
                  <JsonViewer
                    label="Last priced request JSON"
                    text={completed ? JSON.stringify(completed.request) : null}
                  />
                </TabsContent>
                <TabsContent value="scenarios" className="print:hidden">
                  {completed?.instrumentType === "structured_credit" ? (
                    <StructuredCreditPanel
                      key={completed.request.instrumentJson}
                      request={{
                        instrumentJson: completed.request.instrumentJson,
                        marketJson: completed.request.marketJson,
                        asOf: completed.request.asOf,
                      }}
                      seeds={scenario}
                      scenariosOpen={scenariosOpen}
                      onScenariosOpenChange={setScenariosOpen}
                      density={currentDensity}
                    />
                  ) : (
                    <p>Scenarios are unavailable for this instrument.</p>
                  )}
                </TabsContent>
              </Tabs>
            </section>
            <footer className="finstack-workbench__status print:hidden">
              <span>
                {shown ? (
                  <>
                    priced{" "}
                    <span className="font-mono">{shown.request.asOf}</span>
                  </>
                ) : (
                  "Awaiting valuation"
                )}
              </span>
              <span>
                model{" "}
                <span className="font-mono">
                  {resolveModel(shown?.request.model ?? params.model)}
                </span>
              </span>
              {typeof meta?.numeric_mode === "string" && (
                <span className="font-mono">{meta.numeric_mode}</span>
              )}
              {typeof rounding === "string" && <span>{rounding}</span>}
              {shown?.result.meta?.version ? (
                <span>
                  finstack-quant-wasm{" "}
                  <span className="font-mono">
                    {String(shown.result.meta.version)}
                  </span>
                </span>
              ) : null}
              <span className="ml-auto">
                <span className="finstack-kbd">1</span>
                <span className="finstack-kbd">2</span>inputs{" "}
                <span className="finstack-kbd">3</span>
                <span className="finstack-kbd">4</span>
                <span className="finstack-kbd">5</span>results
              </span>
            </footer>
          </section>
        </Tabs>
      </div>
    </SurfaceContext.Provider>
  );
}
