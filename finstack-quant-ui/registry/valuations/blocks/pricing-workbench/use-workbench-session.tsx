"use client";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { ValuationResult } from "finstack-quant-wasm";
import {
  jsonError,
  type PricingParams,
} from "@/components/finstack/valuations/components/pricing-params-form/pricing-params-form";
import { PricingParamsForm } from "@/components/finstack/valuations/components/pricing-params-form/pricing-params-form";
import { marketModule } from "@/components/finstack/core/components/market-context-form/market";
import { useCashflows } from "@/hooks/valuations/use-cashflows/use-cashflows";
import { useMarketValidator } from "@/hooks/core/use-market-validator/use-market-validator";
import { useFinstack } from "@/hooks/shared/use-finstack/use-finstack";
import { useInstrumentValidator } from "@/hooks/valuations/use-instrument-validator/use-instrument-validator";
import {
  useModels,
  useMetrics,
  useMetricMetadata,
  useMoneyFormat,
  usePriceInstrument,
  type PriceRequest,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { returnedRounding } from "@/components/finstack/valuations/primitives/stamp-badge/stamp-badge";
import { resolveModel } from "@/workers/finstack-contract";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { workbenchStatus } from "./workbench-status";
import { usePrintSnapshot, usePrintWhenReady } from "./use-print-report";

/** Models accepted by the native cashflow exporter; "default" is not resolved there. */
const CASHFLOW_MODELS = new Set(["discounting", "hazard_rate"]);

function instrumentTypeOf(json: string): string {
  try {
    const value = JSON.parse(json).instrument?.type;
    return typeof value === "string" ? value : "";
  } catch {
    return "";
  }
}

export interface WorkbenchSessionInput {
  defaultRequest: PriceRequest;
  density: "compact" | "comfortable";
  defaultInstrumentType?: string;
  defaultResultTab: "cashflows" | "measures" | "buckets" | "diagnostics";
  exampleRequests?: Readonly<Record<string, PriceRequest>>;
  exampleResultTabs?: Readonly<
    Record<string, "cashflows" | "measures" | "buckets" | "diagnostics">
  >;
  onInstrumentTypeChange?: (type: string) => void;
}

/** Request, market, and tab state for one workbench mount. Remount to load another document. */
export function useWorkbenchSession({
  defaultRequest,
  density,
  defaultInstrumentType,
  defaultResultTab,
  exampleRequests,
  exampleResultTabs,
  onInstrumentTypeChange,
}: WorkbenchSessionInput) {
  const [initial] = useState(() => structuredClone(defaultRequest));
  const [initialType] = useState(() =>
    instrumentTypeOf(initial.instrumentJson),
  );
  const [type, setType] = useState(defaultInstrumentType ?? initialType);
  const [instrumentRevision, setInstrumentRevision] = useState(0);
  const [scenariosOpen, setScenariosOpen] = useState(false);
  const [resultTab, setResultTab] = useState<string>(defaultResultTab);
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
    instrumentType: string;
  } | null>(null);
  const selectType = useCallback(
    (nextType: string) => {
      const example = exampleRequests?.[nextType];
      if (example) {
        setInstrument(null);
        setMarket(null);
        setMarketReady(false);
        setMarketDocument((current) => ({
          json: example.marketJson,
          revision: current.revision + 1,
        }));
        setParams({
          asOf: example.asOf,
          model: example.model,
          metrics: example.metrics,
          pricingOptions: example.pricingOptions,
          marketHistory: example.marketHistory,
        });
        setRequest(null);
        setCompleted(null);
        setInstrumentRevision((current) => current + 1);
        setResultTab(exampleResultTabs?.[nextType] ?? defaultResultTab);
      }
      setType(nextType);
      onInstrumentTypeChange?.(nextType);
    },
    [
      defaultResultTab,
      exampleRequests,
      exampleResultTabs,
      onInstrumentTypeChange,
    ],
  );
  const { printSnapshot, startPrint, printing } = usePrintSnapshot(completed);
  const shown = printSnapshot ?? completed;
  const measureMetadata = useMetricMetadata(
    Object.keys(shown?.result.measures ?? {}),
    shown !== null,
  );
  const moneyFormat = useMoneyFormat(
    shown
      ? {
          value: shown.result.value,
          rounding: returnedRounding(shown.result.meta),
        }
      : null,
  );
  const shownModel = shown?.request.model;
  const cashflowModel =
    shownModel && CASHFLOW_MODELS.has(shownModel) ? shownModel : null;
  const cashflowRequest = shown?.request ?? initial;
  const cashflows = useCashflows(
    {
      instrumentJson: cashflowRequest.instrumentJson,
      marketJson: cashflowRequest.marketJson,
      asOf: cashflowRequest.asOf,
      model: cashflowModel ?? "",
    },
    cashflowModel !== null,
  );
  // A disabled query stays pending forever; only an active cashflow request can delay printing.
  const preparing =
    cashflows.isLoading ||
    measureMetadata.isPending ||
    measureMetadata.isFetching ||
    moneyFormat.isPending ||
    moneyFormat.isFetching;
  usePrintWhenReady(printSnapshot, preparing);
  const printMarket = useMemo(
    () =>
      printSnapshot
        ? (marketModule.codec.parse(
            printSnapshot.request.marketJson,
          ) as MarketContextStateWire)
        : null,
    [printSnapshot],
  );
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
    if (request && price.data)
      setCompleted({
        request,
        result: price.data,
        instrumentType: instrumentTypeOf(request.instrumentJson),
      });
  }, [request, price.data]);
  const { stale, status, stateKind, currentError } = workbenchStatus({
    workerStatus: worker.status,
    candidate,
    request,
    completed,
    fetching: price.isFetching,
    errorMessage: price.error?.message,
  });
  const meta = shown?.result.meta;
  const rounding = meta?.rounding.mode;
  const measureGridProps = {
    result: shown?.result,
    featuredMetrics: shown?.request.metrics ?? undefined,
    model: shown ? resolveModel(shown.request.model) : undefined,
    formattedValue: moneyFormat.data,
    metadata: measureMetadata.data,
    density: currentDensity,
    loading: !printSnapshot && price.isFetching,
    error: printSnapshot
      ? undefined
      : (currentError ??
        measureMetadata.error?.message ??
        moneyFormat.error?.message),
  };
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

  return {
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
    moneyFormat,
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
  };
}
