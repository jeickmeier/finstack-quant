"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  QuoteConverter,
  type QuoteConverterProps,
  type QuoteInput,
} from "@/components/finstack/valuations/components/quote-converter/quote-converter";
import {
  useBondQuoteSetFieldUnits,
  useComputeQuotes,
} from "@/hooks/valuations/use-compute-quotes/use-compute-quotes";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const { currency, cases } = data.quoteConverter;
const { callable } = cases;
const start = callable.request.quoteInput as QuoteInput;
// Host display choice: six decimal places of the returned values.
const sixPlaces = (value: number) => value.toFixed(6);
function Live({ density }: ExampleProps) {
  const [quoteInput, setQuoteInput] = useState<QuoteInput>(start);
  const quotes = useComputeQuotes({ ...callable.request, quoteInput });
  const units = useBondQuoteSetFieldUnits(currency);
  return (
    <QuoteConverter
      defaultInput={start}
      onSubmit={setQuoteInput}
      quotes={(quotes.data as QuoteConverterProps["quotes"]) ?? null}
      units={units.data ?? null}
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
