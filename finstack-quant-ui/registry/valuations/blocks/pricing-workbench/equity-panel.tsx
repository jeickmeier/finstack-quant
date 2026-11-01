"use client";
import { useState } from "react";
import {
  useEquityMetrics,
  useEquityMetricsFieldUnits,
  type DealSimulationRequest,
} from "@/hooks/valuations/use-deal-report/use-deal-report";
import { EquityTrancheCard } from "@/components/finstack/valuations/components/equity-tranche-card/equity-tranche-card";
/**
 * Native equity metrics of the completed request's deal, in the units Rust states for
 * their currency. The call is always made: Rust decides whether the deal has an equity
 * tranche. A blank price is omitted, so Rust's default applies.
 */
export function EquityPanel({
  request,
  density,
}: {
  request: DealSimulationRequest;
  density?: "compact" | "comfortable";
}) {
  const [purchasePrice, setPurchasePrice] = useState("");
  const metrics = useEquityMetrics({
    ...request,
    purchasePricePct:
      purchasePrice.trim() === "" ? null : Number(purchasePrice),
  });
  const units = useEquityMetricsFieldUnits(metrics.data?.currency ?? null);
  return (
    <EquityTrancheCard
      metrics={metrics.data ?? null}
      units={units.data ?? null}
      purchasePrice={purchasePrice}
      onPurchasePriceChange={setPurchasePrice}
      loading={metrics.isFetching || units.isFetching}
      error={metrics.error?.message ?? units.error?.message}
      density={density}
    />
  );
}
