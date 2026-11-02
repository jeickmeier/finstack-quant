"use client";
import { useState } from "react";
import {
  useMetricMetadata,
  type PriceRequest,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useConvertibleSpotLadder } from "@/hooks/valuations/use-convertible-spot-ladder/use-convertible-spot-ladder";
import {
  ConvertibleCard,
  convertibleMetrics,
  ladderMetrics,
  type ConvertibleCardProps,
} from "@/components/finstack/valuations/components/convertible-card/convertible-card";
import { useInstrumentMetrics } from "./instrument-metrics";
/** The completed request's convertible by share price, and its metrics by id. */
export function ConvertiblePanel({
  request,
  density,
}: {
  request: PriceRequest;
  density?: "compact" | "comfortable";
}) {
  const [spots, setSpots] = useState<readonly number[] | null>(null);
  const card = useInstrumentMetrics(
    "convertible_bond",
    convertibleMetrics,
    request,
  );
  const ladder = useConvertibleSpotLadder({
    instrumentJson: request.instrumentJson,
    marketJson: request.marketJson,
    asOf: request.asOf,
    spots,
  });
  const metadata = useMetricMetadata(ladderMetrics);
  return (
    <ConvertibleCard
      {...card}
      ladder={(ladder.data as ConvertibleCardProps["ladder"]) ?? null}
      ladderMetadata={metadata.data}
      ladderLoading={ladder.isFetching}
      ladderError={ladder.error?.message}
      onSpotsChange={setSpots}
      density={density}
    />
  );
}
