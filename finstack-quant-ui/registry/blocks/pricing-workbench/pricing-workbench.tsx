"use client";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import type { ValuationResult } from "finstack-quant-wasm";
import { InstrumentForm } from "@/components/finstack/components/instrument-form/instrument-form";
import {
  PricingParamsForm,
  jsonError,
  type PricingParams,
} from "@/components/finstack/components/pricing-params-form/pricing-params-form";
import { MarketContextForm } from "@/components/finstack/components/market-context-form/market-context-form";
import { marketModule } from "@/components/finstack/components/market-context-form/market";
import { CashflowViewer } from "@/components/finstack/components/cashflow-viewer/cashflow-viewer";
import { useCashflows } from "@/hooks/use-cashflows/use-cashflows";
import { ExplanationTrace } from "@/components/finstack/components/explanation-trace/explanation-trace";
import { ValuationDetails } from "@/components/finstack/components/valuation-details/valuation-details";
import {
  MarketContextBrowser,
  type MarketContextBrowserProps,
} from "@/components/finstack/components/market-context-browser/market-context-browser";
import {
  VolCubeExplorer,
  type VolCubeExplorerProps,
} from "@/components/finstack/components/vol-cube-explorer/vol-cube-explorer";
import {
  FxSurfaceChart,
  type FxSurfaceChartProps,
} from "@/components/finstack/components/fx-surface-chart/fx-surface-chart";
import { useMarketValidator } from "@/hooks/use-market-validator/use-market-validator";
import { ScenarioPanel } from "./scenario-panel";
import { CalibrationPanel } from "./calibration-panel";
import type { CalibrationFitChartProps } from "@/components/finstack/components/calibration-fit-chart/calibration-fit-chart";
import { MeasuresGrid } from "@/components/finstack/components/measures-grid/measures-grid";
import { JsonViewer } from "@/components/finstack/primitives/json-viewer/json-viewer";
import { useFinstack } from "@/hooks/use-finstack/use-finstack";
import { useInstrumentValidator } from "@/hooks/use-instrument-validator/use-instrument-validator";
import {
  useModels,
  useMetrics,
  usePriceInstrument,
  type PriceRequest,
} from "@/hooks/use-price-instrument/use-price-instrument";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import fixture from "@/lib/finstack/fixtures/results/bond.json";
/** Embed inside FinstackQueryProvider, or an existing QueryClientProvider + FinstackProvider. The host owns the route. */
export function PricingWorkbench({
  defaultRequest = fixture.request,
  density = "compact",
  defaultInstrumentType,
  defaultCalibrationJson,
  surfaceOptions,
  cubeOptions,
  fxOptions,
  calibrationChartOptions,
  scenario,
}: {
  /** Complete initial instrument request and canonical market snapshot. Remount to load another document. */
  defaultRequest?: PriceRequest;
  density?: "compact" | "comfortable";
  /** Host-owned deep link selects this canonical example; supplied market/parameters stay explicit. */
  defaultInstrumentType?: string;
  defaultCalibrationJson?: string;
  /** Explicit native tranche/grid inputs and display extent; evaluated on the completed structured-credit request only. */
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
  const [initial] = useState(() => structuredClone(defaultRequest));
  const [initialType] = useState(() => {
    try {
      const value = JSON.parse(initial.instrumentJson).instrument?.type;
      return typeof value === "string" ? value : "";
    } catch {
      return "";
    }
  });
  const [type, setType] = useState(defaultInstrumentType ?? initialType);
  const [scenariosOpen, setScenariosOpen] = useState(false);
  const [resultTab, setResultTab] = useState("cashflows");
  const [currentDensity, setCurrentDensity] = useState(density);
  const [theme, setTheme] = useState<"light" | "dark" | undefined>();
  const [instrumentOpen, setInstrumentOpen] = useState(false);
  const [activeTab, setActiveTab] = useState("instrument");
  const [marketTab, setMarketTab] = useState("view");
  const [marketDocument, setMarketDocument] = useState({
    json: initial.marketJson,
    revision: 0,
  });
  const [marketReady, setMarketReady] = useState(false);
  const [market, setMarket] = useState<{
    json: string;
    state: MarketContextStateWire;
  } | null>(null);
  const acceptMarket = useCallback((json: string | null) => {
    setMarketReady(json !== null);
    if (json !== null)
      setMarket((current) =>
        current?.json === json
          ? current
          : {
              json,
              state: marketModule.codec.parse(json) as MarketContextStateWire,
            },
      );
  }, []);
  const [instrument, setInstrument] = useState<string | null>(null);
  const [params, setParams] = useState<PricingParams>(() => ({
    asOf: initial.asOf,
    model: initial.model,
    metrics: initial.metrics,
    pricingOptions: initial.pricingOptions,
    marketHistory: initial.marketHistory,
  }));
  const [request, setRequest] = useState<PriceRequest | null>(null);
  const [completed, setCompleted] = useState<{
    request: PriceRequest;
    result: ValuationResult;
  } | null>(null);
  const [printSnapshot, setPrintSnapshot] = useState<typeof completed>(null);
  const shown = printSnapshot ?? completed;
  const printRequest = printSnapshot?.request ?? initial;
  const printCashflows = useCashflows(
    {
      instrumentJson: printRequest.instrumentJson,
      marketJson: printRequest.marketJson,
      asOf: printRequest.asOf,
      model: printRequest.model ?? "default",
    },
    printSnapshot !== null,
  );
  const printMarket = useMemo(
    () =>
      printSnapshot
        ? (marketModule.codec.parse(
            printSnapshot.request.marketJson,
          ) as MarketContextStateWire)
        : null,
    [printSnapshot],
  );
  useEffect(() => {
    const finish = () => setPrintSnapshot(null);
    window.addEventListener("afterprint", finish);
    return () => window.removeEventListener("afterprint", finish);
  }, []);
  useEffect(() => {
    if (!printSnapshot || printCashflows.isPending || printCashflows.isFetching)
      return;
    let active = true;
    void document.fonts.ready.then(() =>
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (active) window.print();
        }),
      ),
    );
    return () => {
      active = false;
    };
  }, [printSnapshot, printCashflows.isPending, printCashflows.isFetching]);
  const worker = useFinstack();
  const validate = useInstrumentValidator();
  const validateMarket = useMarketValidator();
  const models = useModels(),
    metrics = useMetrics();
  const candidate = useMemo<PriceRequest | null>(
    () =>
      instrument &&
      marketReady &&
      market &&
      !jsonError(params.pricingOptions) &&
      !jsonError(params.marketHistory)
        ? {
            instrumentJson: instrument,
            marketJson: market.json,
            ...params,
          }
        : null,
    [instrument, market, marketReady, params],
  );
  useEffect(() => {
    if (!candidate) return;
    const timer = setTimeout(() => setRequest(candidate), 250);
    return () => clearTimeout(timer);
  }, [candidate]);
  const price = usePriceInstrument(
    request ?? initial,
    request !== null && request === candidate,
  );
  useEffect(() => {
    if (request && price.data) setCompleted({ request, result: price.data });
  }, [request, price.data]);
  const currentError = request === candidate ? price.error?.message : undefined;
  const status =
    worker.status !== "ready"
      ? `Worker ${worker.status}`
      : !candidate
        ? "Waiting for valid inputs"
        : request !== candidate
          ? "Pricing queued"
          : price.isFetching
            ? "Pricing…"
            : currentError
              ? "Pricing failed"
              : completed
                ? "Priced"
                : "Waiting for valuation";
  const pricingContext = (
    <PricingParamsForm
      value={params}
      onValueChange={setParams}
      instrumentType={type}
      models={models.data ?? {}}
      metrics={metrics.data ?? {}}
      loading={models.isPending || metrics.isPending}
      error={models.error?.message ?? metrics.error?.message}
      sections={["context"]}
    />
  );
  return (
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
              target.closest("[data-finstack-print]") !== event.currentTarget ||
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
          data-printing={
            printSnapshot
              ? printCashflows.isPending || printCashflows.isFetching
                ? "preparing"
                : "ready"
              : undefined
          }
          data-theme={printSnapshot ? "light" : theme}
          data-density={currentDensity}
          className="finstack-workbench font-sans text-sm text-foreground focus-visible:outline-2 focus-visible:outline-ring"
        >
          <header className="finstack-workbench__bar print:hidden">
            <h2 className="finstack-workbench__brand">finstack</h2>
            <TabsList
              aria-label="Pricing inputs"
              className="finstack-workbench__tabs"
            >
              <TabsTrigger value="instrument">1 Instrument</TabsTrigger>
              <TabsTrigger value="market">2 Market</TabsTrigger>
              <TabsTrigger value="calibrate">Calibrate</TabsTrigger>
            </TabsList>
            <div className="finstack-workbench__context">{pricingContext}</div>
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
            <Popover>
              <PopoverTrigger render={<Button variant="outline" size="sm" />}>
                Settings
              </PopoverTrigger>
              <PopoverContent align="end">
                <div className="max-h-[60vh] space-y-4 overflow-auto">
                  <PricingParamsForm
                    value={params}
                    onValueChange={setParams}
                    instrumentType={type}
                    models={models.data ?? {}}
                    metrics={metrics.data ?? {}}
                    loading={models.isPending || metrics.isPending}
                    error={models.error?.message ?? metrics.error?.message}
                    sections={["metrics", "advanced"]}
                  />
                  <Label className="flex items-center justify-between gap-3">
                    Density
                    <Select
                      items={[
                        { value: "compact", label: "Compact" },
                        { value: "comfortable", label: "Comfortable" },
                      ]}
                      value={currentDensity}
                      onValueChange={(value) => {
                        if (value !== "compact" && value !== "comfortable")
                          return;
                        document.documentElement.dataset.density = value;
                        setCurrentDensity(value);
                      }}
                    >
                      <SelectTrigger aria-label="Workbench density">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="compact">Compact</SelectItem>
                        <SelectItem value="comfortable">Comfortable</SelectItem>
                      </SelectContent>
                    </Select>
                  </Label>
                  <Button
                    variant="outline"
                    size="sm"
                    type="button"
                    onClick={(event) => {
                      const inherited = event.currentTarget
                        .closest("[data-theme]")
                        ?.getAttribute("data-theme");
                      const nextTheme =
                        (theme ?? inherited) === "dark" ? "light" : "dark";
                      document.documentElement.dataset.theme = nextTheme;
                      document.documentElement.classList.toggle(
                        "dark",
                        nextTheme === "dark",
                      );
                      setTheme(nextTheme);
                    }}
                  >
                    Toggle theme
                  </Button>
                </div>
              </PopoverContent>
            </Popover>
            <Button
              variant="outline"
              size="sm"
              type="button"

              disabled={
                !completed || status !== "Priced" || printSnapshot !== null
              }
              onClick={() => setPrintSnapshot(completed)}
            >
              {printSnapshot ? "Preparing report…" : "Print report"}
            </Button>{" "}
            <p
              role="status"
              aria-live="polite"
              className="finstack-workbench__state"
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
                type={type}
                onTypeChange={setType}
                defaultJson={
                  (defaultInstrumentType ?? initialType) === initialType
                    ? initial.instrumentJson
                    : undefined
                }
                validate={validate}
                onValidated={setInstrument}
                onSubmit={setInstrument}
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
                  aria-label="Market mode"
                  className="finstack-workbench__tabs mb-2 print:hidden"
                >
                  <TabsTrigger value="view">View market</TabsTrigger>
                  <TabsTrigger value="edit">Edit market</TabsTrigger>
                  <TabsTrigger value="json">Snapshot JSON</TabsTrigger>
                </TabsList>
                <TabsContent value="edit" keepMounted className="print:hidden">
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
                          const cube = (printMarket ?? market.state).vol_cubes[
                              index
                            ]!,
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
          <section aria-label="Results" className="finstack-workbench__results">
            <div className="finstack-workbench__summary">
              <header className="finstack-workbench__results-header">
                <h2 className="text-sm font-medium">Results</h2>
                <span className="text-xs text-muted-foreground">
                  {shown
                    ? `${Object.keys(shown.result.measures).length} metrics · grouped`
                    : "Awaiting valuation"}
                </span>
              </header>
              <div className="finstack-workbench__summary-body">
                <MeasuresGrid
                  result={shown?.result}
                  model={shown ? (shown.request.model ?? "default") : undefined}
                  groups={metrics.data ?? {}}
                  density={currentDensity}
                  loading={!printSnapshot && price.isFetching}
                  error={printSnapshot ? undefined : currentError}
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
                  <TabsTrigger value="cashflows">3 Cashflows</TabsTrigger>
                  <TabsTrigger value="diagnostics">4 Diagnostics</TabsTrigger>
                  <TabsTrigger value="trace">5 Trace</TabsTrigger>
                  <TabsTrigger value="request">Request</TabsTrigger>
                  {scenario && (
                    <TabsTrigger value="scenarios">Scenarios</TabsTrigger>
                  )}
                </TabsList>
              </div>
              <TabsContent value="cashflows" keepMounted>
                {shown ? (
                  <CashflowViewer
                    density={currentDensity}
                    request={{
                      instrumentJson: shown.request.instrumentJson,
                      marketJson: shown.request.marketJson,
                      asOf: shown.request.asOf,
                      model: shown.request.model ?? "default",
                    }}
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
                completed &&
                JSON.parse(completed.request.instrumentJson).instrument
                  ?.type === "structured_credit" ? (
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
              {shown ? `Priced ${shown.request.asOf}` : "Awaiting valuation"}
            </span>
            <span>
              model {shown?.request.model ?? params.model ?? "default"}
            </span>
            <span className="font-mono">
              {shown?.result.instrument_id ?? type}
            </span>
            <span>
              {shown?.result.meta?.version
                ? `WASM ${String(shown.result.meta.version)}`
                : ""}
            </span>
          </footer>
        </section>
      </Tabs>
    </div>
  );
}
