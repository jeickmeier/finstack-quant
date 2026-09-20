"use client";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Tabs } from "@base-ui/react/tabs";
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
  const [cashflowsOpen, setCashflowsOpen] = useState(false);
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
  return (
    <section
      aria-label="Pricing workbench"
      data-finstack-print=""
      data-printing={
        printSnapshot
          ? printCashflows.isPending || printCashflows.isFetching
            ? "preparing"
            : "ready"
          : undefined
      }
      data-theme={printSnapshot ? "light" : undefined}
      data-density={density}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-lg font-semibold">Pricing workbench</h2>
        <p role="status" aria-live="polite">
          {status}
        </p>
        <button
          type="button"
          className="rounded-sm border border-border px-3 py-1 print:hidden"
          disabled={!completed || status !== "Priced" || printSnapshot !== null}
          onClick={() => setPrintSnapshot(completed)}
        >
          {printSnapshot ? "Preparing report…" : "Print report"}
        </button>
      </header>
      {worker.error && (
        <p role="alert" className="text-error">
          {worker.error.message}
        </p>
      )}
      {shown && (
        <details
          open={instrumentOpen || printSnapshot !== null}
          onToggle={(event) => {
            if (!printSnapshot) setInstrumentOpen(event.currentTarget.open);
          }}
        >
          <summary>Priced instrument</summary>
          <JsonViewer
            label="Priced instrument JSON"
            text={shown.request.instrumentJson}
            density={density}
          />
        </details>
      )}
      <Tabs.Root
        value={printSnapshot ? "market" : activeTab}
        onValueChange={setActiveTab}
      >
        <Tabs.List
          aria-label="Pricing inputs"
          className="mb-3 flex gap-2 border-b border-border print:hidden"
        >
          <Tabs.Tab
            value="instrument"
            className="border-b-2 border-transparent px-3 py-2 data-active:border-primary focus-visible:outline-2 focus-visible:outline-ring"
          >
            1 Instrument
          </Tabs.Tab>
          <Tabs.Tab
            value="market"
            className="border-b-2 border-transparent px-3 py-2 data-active:border-primary focus-visible:outline-2 focus-visible:outline-ring"
          >
            2 Market
          </Tabs.Tab>
          <Tabs.Tab
            value="calibrate"
            className="border-b-2 border-transparent px-3 py-2 data-active:border-primary focus-visible:outline-2 focus-visible:outline-ring"
          >
            3 Calibrate
          </Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel
          value="instrument"
          keepMounted
          className="max-h-[55vh] overflow-y-auto pr-2 print:hidden"
        >
          <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
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
            <PricingParamsForm
              value={params}
              onValueChange={setParams}
              instrumentType={type}
              models={models.data ?? {}}
              metrics={metrics.data ?? {}}
              loading={models.isPending || metrics.isPending}
              error={models.error?.message ?? metrics.error?.message}
            />
          </div>
        </Tabs.Panel>
        <Tabs.Panel
          value="market"
          keepMounted
          className="max-h-[55vh] space-y-3 overflow-y-auto pr-2"
        >
          <Tabs.Root
            value={printSnapshot ? "view" : marketTab}
            onValueChange={setMarketTab}
          >
            <Tabs.List
              aria-label="Market mode"
              className="flex gap-2 print:hidden"
            >
              <Tabs.Tab
                value="view"
                className="rounded-sm border border-border px-3 py-1 data-active:bg-accent"
              >
                View
              </Tabs.Tab>
              <Tabs.Tab
                value="edit"
                className="rounded-sm border border-border px-3 py-1 data-active:bg-accent"
              >
                Edit
              </Tabs.Tab>
            </Tabs.List>
            <Tabs.Panel value="edit" keepMounted className="print:hidden">
              <MarketContextForm
                key={marketDocument.revision}
                defaultJson={marketDocument.json}
                validate={validateMarket}
                onValidated={acceptMarket}
                onSubmit={acceptMarket}
              />
            </Tabs.Panel>
            <Tabs.Panel value="view" keepMounted>
              <JsonViewer
                label="Market snapshot"
                text={
                  printSnapshot?.request.marketJson ??
                  market?.json ??
                  initial.marketJson
                }
                density={density}
              />
              {market && (
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
              )}
            </Tabs.Panel>
          </Tabs.Root>
        </Tabs.Panel>
        <Tabs.Panel
          value="calibrate"
          keepMounted
          className="max-h-[55vh] space-y-3 overflow-y-auto pr-2 print:hidden"
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
        </Tabs.Panel>
      </Tabs.Root>
      <section
        aria-label="Results"
        className="space-y-3 border-t border-border pt-3"
      >
        <p className="text-xs text-muted-foreground">
          Results reflect the last completed request.
        </p>
        <MeasuresGrid
          result={shown?.result}
          groups={metrics.data ?? {}}
          density={density}
          loading={!printSnapshot && price.isFetching}
          error={printSnapshot ? undefined : currentError}
        />
        {shown && <ValuationDetails result={shown.result} density={density} />}
        <details
          open={cashflowsOpen || printSnapshot !== null}
          onToggle={(event) => {
            if (!printSnapshot) setCashflowsOpen(event.currentTarget.open);
          }}
        >
          <summary>Cashflows</summary>
          {shown && (cashflowsOpen || printSnapshot !== null) && (
            <CashflowViewer
              density={density}
              request={{
                instrumentJson: shown.request.instrumentJson,
                marketJson: shown.request.marketJson,
                asOf: shown.request.asOf,
                model: shown.request.model ?? "default",
              }}
            />
          )}
        </details>
        {scenario &&
          completed &&
          JSON.parse(completed.request.instrumentJson).instrument?.type ===
            "structured_credit" && (
            <details
              className="print:hidden"
              onToggle={(event) => setScenariosOpen(event.currentTarget.open)}
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
          )}
        <details className="print:hidden">
          <summary>Last priced request</summary>
          <JsonViewer
            label="Last priced request JSON"
            text={completed ? JSON.stringify(completed.request) : null}
          />
        </details>
      </section>
    </section>
  );
}
