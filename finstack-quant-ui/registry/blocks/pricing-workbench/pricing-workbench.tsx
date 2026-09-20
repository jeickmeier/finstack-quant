"use client";
import { useEffect, useMemo, useState } from "react";
import { Tabs } from "@base-ui/react/tabs";
import type { ValuationResult } from "finstack-quant-wasm";
import { InstrumentForm } from "@/components/finstack/components/instrument-form/instrument-form";
import {
  PricingParamsForm,
  jsonError,
  type PricingParams,
} from "@/components/finstack/components/pricing-params-form/pricing-params-form";
import { CurveChart } from "@/components/finstack/components/curve-chart/curve-chart";
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
import { createWireCodec, serializeHost } from "@/lib/finstack/codec.mjs";
import marketSchema from "@/lib/finstack/generated/schemas/market_context_state.json";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import fixture from "@/lib/finstack/fixtures/results/bond.json";
/** Embed inside FinstackQueryProvider, or an existing QueryClientProvider + FinstackProvider. The host owns the route. */
export function PricingWorkbench({
  defaultRequest = fixture.request,
  density = "compact",
}: {
  /** Initial bond request and canonical market snapshot. Remount to load another document. */
  defaultRequest?: PriceRequest;
  density?: "compact" | "comfortable";
}) {
  const [initial] = useState(() => structuredClone(defaultRequest));
  const [type, setType] = useState("bond");
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
  const models = useModels(),
    metrics = useMetrics();
  const market = useMemo(() => {
    try {
      return {
        state: createWireCodec(marketSchema).parse(
          initial.marketJson,
        ) as MarketContextStateWire,
        error: undefined,
      };
    } catch (error) {
      return {
        state: undefined,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }, [initial.marketJson]);
  const candidate = useMemo<PriceRequest | null>(
    () =>
      type === "bond" &&
      instrument &&
      market.state &&
      !jsonError(params.pricingOptions) &&
      !jsonError(params.marketHistory)
        ? {
            instrumentJson: instrument,
            marketJson: initial.marketJson,
            ...params,
          }
        : null,
    [type, instrument, market.state, initial.marketJson, params],
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
      : type !== "bond"
        ? "Instrument not yet supported"
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
              defaultJson={initial.instrumentJson}
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
          <JsonViewer
            label="Market snapshot"
            text={initial.marketJson}
            error={market.error}
          />
          {market.state && <CurveChart curves={market.state.curves} />}
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
        <details>
          <summary>Result JSON</summary>
          <JsonViewer
            label="Valuation result JSON"
            text={completed ? serializeHost(completed.result) : null}
          />
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
