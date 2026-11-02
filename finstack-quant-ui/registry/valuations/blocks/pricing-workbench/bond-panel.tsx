"use client";
import { useState } from "react";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import type { PriceRequest } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useBondWorkoutLadder } from "@/hooks/valuations/use-bond-workout-ladder/use-bond-workout-ladder";
import {
  useBondQuoteSetFieldUnits,
  useComputeQuotes,
} from "@/hooks/valuations/use-compute-quotes/use-compute-quotes";
import { useMetricMetadata } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import {
  BondCreditCard,
  bondCreditMetrics,
} from "@/components/finstack/valuations/components/bond-credit-card/bond-credit-card";
import {
  workoutLadderMetrics,
  YieldWorkoutLadder,
  type YieldWorkoutLadderProps,
} from "@/components/finstack/valuations/components/yield-workout-ladder/yield-workout-ladder";
import {
  QuoteConverter,
  type QuoteConverterProps,
  type QuoteInput,
} from "@/components/finstack/valuations/components/quote-converter/quote-converter";
import { useInstrumentMetrics } from "./instrument-metrics";
import { LazySection } from "./lazy-section";
type Density = "compact" | "comfortable";
/** The form starts from the schema's Rust example and prices nothing until submitted. */
const firstQuote = contracts.bond_quote_input.schema.examples[0] as QuoteInput;
function Credit({
  request,
  density,
}: {
  request: PriceRequest;
  density?: Density;
}) {
  const [overrides, setOverrides] = useState(request.metricPricingOverrides);
  const card = useInstrumentMetrics("bond", bondCreditMetrics, {
    ...request,
    metricPricingOverrides: overrides,
  });
  return (
    <BondCreditCard
      {...card}
      density={density}
      overrides={overrides}
      onOverridesChange={setOverrides}
    />
  );
}
function Workout({
  request,
  density,
}: {
  request: PriceRequest;
  density?: Density;
}) {
  const ladder = useBondWorkoutLadder({
    instrumentJson: request.instrumentJson,
    marketJson: request.marketJson,
    asOf: request.asOf,
  });
  const metadata = useMetricMetadata(workoutLadderMetrics);
  return (
    <YieldWorkoutLadder
      ladder={(ladder.data as YieldWorkoutLadderProps["ladder"]) ?? null}
      metadata={metadata.data}
      loading={ladder.isFetching}
      error={ladder.error?.message}
      density={density}
    />
  );
}
function Quotes({
  request,
  density,
}: {
  request: PriceRequest;
  density?: Density;
}) {
  const [quoteInput, setQuoteInput] = useState<QuoteInput | null>(null);
  const quotes = useComputeQuotes(
    quoteInput && {
      instrumentJson: request.instrumentJson,
      marketJson: request.marketJson,
      asOf: request.asOf,
      quoteInput,
    },
  );
  const { currency } = JSON.parse(request.instrumentJson).instrument.spec
    .notional as { currency: string };
  const units = useBondQuoteSetFieldUnits(currency);
  return (
    <QuoteConverter
      defaultInput={firstQuote}
      onSubmit={setQuoteInput}
      quotes={(quotes.data as QuoteConverterProps["quotes"]) ?? null}
      units={units.data ?? null}
      loading={quotes.isFetching}
      error={quotes.error?.message}
      density={density}
    />
  );
}
/**
 * The completed request's bond: each card opens on its own, makes its own native
 * calls and shows its own error, so one that fails or takes long leaves the others.
 */
export function BondPanel({
  request,
  density,
}: {
  request: PriceRequest;
  density?: Density;
}) {
  return (
    <div className="flex flex-col gap-3">
      <LazySection title="Spreads, risk and carry">
        <Credit request={request} density={density} />
      </LazySection>
      <LazySection title="Yield workout ladder">
        <Workout request={request} density={density} />
      </LazySection>
      <LazySection title="Quote converter">
        <Quotes request={request} density={density} />
      </LazySection>
    </div>
  );
}
