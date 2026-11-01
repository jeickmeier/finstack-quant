"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  QuoteConverter,
  quoteSetMetrics,
  type QuoteConverterProps,
  type QuoteInput,
} from "@/components/finstack/valuations/components/quote-converter/quote-converter";
import { useComputeQuotes } from "@/hooks/valuations/use-compute-quotes/use-compute-quotes";
import { useMetricMetadata } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const { callable } = data.quoteConverter.cases;
const start = callable.request.quoteInput as QuoteInput;
// Host display choice: six decimal places of the returned values.
const sixPlaces = (value: number) => value.toFixed(6);
function Live({ density }: ExampleProps) {
  const [quoteInput, setQuoteInput] = useState<QuoteInput>(start);
  const quotes = useComputeQuotes({ ...callable.request, quoteInput });
  const metadata = useMetricMetadata(quoteSetMetrics);
  return (
    <QuoteConverter
      defaultInput={start}
      onSubmit={setQuoteInput}
      quotes={(quotes.data as QuoteConverterProps["quotes"]) ?? null}
      metadata={metadata.data}
      loading={quotes.isFetching}
      error={quotes.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied set.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
