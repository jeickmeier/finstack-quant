"use client";
import { useState } from "react";
import {
  useTrancheBreakevenCdr,
  useTrancheDiscountMargin,
  useTrancheMetrics,
  useTrancheOas,
  type TrancheRequest,
} from "@/hooks/valuations/use-tranche-analytics/use-tranche-analytics";
import { TrancheAnalytics } from "@/components/finstack/valuations/components/tranche-analytics/tranche-analytics";
/**
 * Native analytics of the completed request's tranche. No price is assumed:
 * metrics use Rust's default target until one is typed, discount margin waits
 * for a price, and OAS runs only when requested at the current price.
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
  const [oasPrice, setOasPrice] = useState<number | null>(null);
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
  const cdr = useTrancheBreakevenCdr(request);
  const dm = useTrancheDiscountMargin(
    marketPricePct === null ? null : { ...request, marketPricePct },
  );
  const oas = useTrancheOas(
    oasPrice === null ? null : { ...request, marketPricePct: oasPrice },
  );
  return (
    <TrancheAnalytics
      trancheId={request.trancheId}
      price={price}
      onPriceChange={setPrice}
      metrics={tile(metricsQuery)}
      breakevenCdr={tile(cdr)}
      discountMargin={tile(dm)}
      oas={
        oasPrice === null || oasPrice === marketPricePct
          ? tile(oas)
          : { error: undefined, value: undefined, loading: false }
      }
      onSolveOas={() => setOasPrice(marketPricePct)}
      density={density}
    />
  );
}
