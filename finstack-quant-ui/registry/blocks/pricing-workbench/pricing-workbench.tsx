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
import { ValuationDetails } from "@/components/finstack/components/valuation-details/valuation-details";
import { FinstackTable } from "@/components/finstack/primitives/finstack-table/finstack-table";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import { useMarketValidator } from "@/hooks/use-market-validator/use-market-validator";
import type { ColumnDef } from "@tanstack/react-table";
import {
  CurveChart,
  curvePanels,
  type CurvePoint,
} from "@/components/finstack/components/curve-chart/curve-chart";
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
const pointKey = (point: CurvePoint) =>
  JSON.stringify([point.curve.type, point.curve.id, point.knot[0]]);
const columns: ColumnDef<{}, CurvePoint, string | number>[] = [
  { id: "curve", header: "Curve", accessorFn: (point) => point.curve.id },
  { id: "type", header: "Variant", accessorFn: (point) => point.curve.type },
  { id: "x", header: "Stored x", accessorFn: (point) => point.knot[0] },
  { id: "value", header: "Stored value", accessorFn: (point) => point.knot[1] },
];
/** Embed inside FinstackQueryProvider, or an existing QueryClientProvider + FinstackProvider. The host owns the route. */
export function PricingWorkbench({
  defaultRequest = fixture.request,
  density = "compact",
  defaultInstrumentType,
}: {
  /** Complete initial instrument request and canonical market snapshot. Remount to load another document. */
  defaultRequest?: PriceRequest;
  density?: "compact" | "comfortable";
  /** Host-owned deep link selects this canonical example; supplied market/parameters stay explicit. */
  defaultInstrumentType?: string;
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
  const [cashflowsOpen, setCashflowsOpen] = useState(false);
  const link = useLinkedSelection();
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
  const points = useMemo(
    () =>
      market
        ? curvePanels(market.state.curves).flatMap((panel) => panel.points)
        : [],
    [market],
  );
  const addresses = useMemo(
    () =>
      new Map(
        points.map((point) => [
          pointKey(point),
          { rowId: pointKey(point), columnId: "value" },
        ]),
      ),
    [points],
  );
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
      data-density={density}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-lg font-semibold">Pricing workbench</h2>
        <p role="status" aria-live="polite">
          {status}
        </p>
      </header>
      {worker.error && (
        <p role="alert" className="text-error">
          {worker.error.message}
        </p>
      )}
      <Tabs.Root defaultValue="instrument">
        <Tabs.List
          aria-label="Pricing inputs"
          className="mb-3 flex gap-2 border-b border-border"
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
        </Tabs.List>
        <Tabs.Panel
          value="instrument"
          keepMounted
          className="max-h-[55vh] overflow-y-auto pr-2"
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
          <Tabs.Root defaultValue="view">
            <Tabs.List aria-label="Market mode" className="flex gap-2">
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
            <Tabs.Panel value="edit" keepMounted>
              <MarketContextForm
                defaultJson={initial.marketJson}
                validate={validateMarket}
                onValidated={acceptMarket}
                onSubmit={acceptMarket}
              />
            </Tabs.Panel>
            <Tabs.Panel value="view" keepMounted>
              <JsonViewer
                label="Market snapshot"
                text={market?.json ?? initial.marketJson}
                density={density}
              />
              {market && (
                <>
                  <FinstackTable
                    caption="Stored curve knots"
                    data={points}
                    columns={columns}
                    getRowId={pointKey}
                    link={link}
                    getRowKey={pointKey}
                    getCellKey={(point, column) =>
                      column === "value" ? pointKey(point) : null
                    }
                    getActiveCell={(key) => addresses.get(key) ?? null}
                    density={density}
                  />
                  <CurveChart
                    curves={market.state.curves}
                    link={{ ...link, getPointKey: pointKey }}
                  />
                </>
              )}
            </Tabs.Panel>
          </Tabs.Root>
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
          result={completed?.result}
          groups={metrics.data ?? {}}
          density={density}
          loading={price.isFetching}
          error={currentError}
        />
        {completed && (
          <ValuationDetails result={completed.result} density={density} />
        )}
        <details
          onToggle={(event) => setCashflowsOpen(event.currentTarget.open)}
        >
          <summary>Cashflows</summary>
          {completed && cashflowsOpen && (
            <CashflowViewer
              density={density}
              request={{
                instrumentJson: completed.request.instrumentJson,
                marketJson: completed.request.marketJson,
                asOf: completed.request.asOf,
                model: completed.request.model ?? "default",
              }}
            />
          )}
        </details>
        <details>
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
