"use client";
import { useState } from "react";
import {
  useTrancheBreakevenCdr,
  useTrancheDiscountMargin,
  useTrancheMetricFieldUnits,
  useTrancheSolveFieldUnits,
  useTrancheMetrics,
  useTrancheOas,
  type TrancheRequest,
} from "@/hooks/valuations/use-tranche-analytics/use-tranche-analytics";
import { TrancheAnalytics } from "@/components/finstack/valuations/components/tranche-analytics/tranche-analytics";
/**
 * Native analytics of the completed request's tranche. No price is assumed:
 * metrics use Rust's default target until one is typed, discount margin waits
 * for a price, and OAS runs only when requested at the current price and the
 * applied OasConfig (Rust's default until one is applied).
 */
export function TranchePanel({
  request,
  metrics,
  density,
}: {
  request: TrancheRequest;
  /** Registry metric ids valued on the note; omitted means none. */
  metrics?: readonly string[];
  density?: "compact" | "comfortable";
}) {
  const [price, setPrice] = useState("");
  // Undefined leaves the configuration to Rust's default OasConfig.
  const [oasConfig, setOasConfig] = useState<string | undefined>();
  const [oasSolve, setOasSolve] = useState<{
    price: number;
    configJson?: string;
  } | null>(null);
  const marketPricePct = price.trim() === "" ? null : Number(price);
  const tile = <T,>(query: {
    data?: T;
    error: Error | null;
    isFetching: boolean;
  }) => ({
    value: query.data,
    error: query.error?.message,
    loading: query.isFetching,
  });
  const metricsQuery = useTrancheMetrics({
    ...request,
    marketPricePct,
    metrics: metrics ?? null,
  });
  const metricUnits = useTrancheMetricFieldUnits(
    metricsQuery.data?.currency ?? null,
  );
  const solveUnits = useTrancheSolveFieldUnits();
  const cdr = useTrancheBreakevenCdr(request);
  const dm = useTrancheDiscountMargin(
    marketPricePct === null ? null : { ...request, marketPricePct },
  );
  const oas = useTrancheOas(
    oasSolve === null
      ? null
      : {
          ...request,
          marketPricePct: oasSolve.price,
          ...(oasSolve.configJson === undefined
            ? {}
            : { configJson: oasSolve.configJson }),
        },
  );
  const solved =
    oasSolve !== null &&
    oasSolve.price === marketPricePct &&
    oasSolve.configJson === oasConfig;
  return (
    <TrancheAnalytics
      trancheId={request.trancheId}
      price={price}
      onPriceChange={setPrice}
      metrics={tile(metricsQuery)}
      metricUnits={tile(metricUnits)}
      solveUnits={tile(solveUnits)}
      breakevenCdr={tile(cdr)}
      discountMargin={tile(dm)}
      oas={
        oasSolve === null || solved
          ? tile(oas)
          : { error: undefined, value: undefined, loading: false }
      }
      onSolveOas={() =>
        marketPricePct !== null &&
        setOasSolve({ price: marketPricePct, configJson: oasConfig })
      }
      onOasConfigChange={setOasConfig}
      density={density}
    />
  );
}
