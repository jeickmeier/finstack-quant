"use client";
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
import {
  FxSurfaceChart,
  type FxSurfaceChartProps,
} from "@/components/finstack/models/components/fx-surface-chart/fx-surface-chart";
import { ScenarioPanel } from "./scenario-panel";
import { CalibrationPanel } from "./calibration-panel";
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
  /** Explicit native tranche/grid inputs and display extent; offered while structured credit is selected and evaluated on its completed request only. */
  scenario?: {
    trancheId: string;
    gridJson: string;
    priceDomain: readonly [number, number];
  };
  surfaceOptions?: MarketContextBrowserProps["surfaceOptions"];
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
    selectType,
    printSnapshot,
    startPrint,
    printing,
    shown,
    measureMetadata,
    cashflows,
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
  } = useWorkbenchSession({
    defaultRequest,
    density,
    defaultInstrumentType,
    defaultResultTab,
    exampleRequests,
    exampleResultTabs,
    onInstrumentTypeChange,
  });
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
            tabIndex={0}
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
              <h2 className="finstack-workbench__brand">finstack</h2>
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
                <InstrumentForm
                  key={instrumentRevision}
                  type={type}
                  onTypeChange={selectType}
                  allowedTypes={
                    exampleRequests ? Object.keys(exampleRequests) : undefined
                  }
                  showGeneratedExample={!exampleRequests}
                  defaultJson={
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
                      revision: current.revision + 1,
                    }));
                  }}
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
                    <TabsTrigger value="request">Request</TabsTrigger>
                    {scenario && type === "structured_credit" && (
                      <TabsTrigger value="scenarios">Scenarios</TabsTrigger>
                    )}
                  </TabsList>
                </div>
                <TabsContent value="measures" className="print:hidden">
                  <MeasuresGrid
                    {...measureGridProps}
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
                      text={cashflows.data}
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
                  {scenario &&
                  completed?.instrumentType === "structured_credit" ? (
                    <details
                      onToggle={(event) =>
                        setScenariosOpen(event.currentTarget.open)
                      }
                    >
                      <summary>Scenario prices</summary>
                      {scenariosOpen && (
                        <ScenarioPanel
                          request={{
                            instrumentJson: completed.request.instrumentJson,
                            marketJson: completed.request.marketJson,
                            asOf: completed.request.asOf,
                            trancheId: scenario.trancheId,
                            gridJson: scenario.gridJson,
                          }}
                          priceDomain={scenario.priceDomain}
                        />
                      )}
                    </details>
                  ) : (
                    <p>Scenario prices are unavailable for this instrument.</p>
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
