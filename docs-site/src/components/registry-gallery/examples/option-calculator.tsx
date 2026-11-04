"use client";
import { useState } from "react";
import { sixPlaces, type ExampleProps } from "./props";
import {
  OptionCalculator,
  type OptionCalculation,
  type OptionCalculationResult,
} from "@/components/finstack/models/components/option-calculator/option-calculator";
import { useOptionCalculator } from "@/hooks/models/use-option-calculator/use-option-calculator";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied result.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Calculator density={density} />
    </WorkerPreview>
  );
}
function Calculator({ density }: ExampleProps) {
  // Starts from the fixture's terms; Calculate replaces the request.
  const [request, setRequest] = useState(
    data.options.black_scholes_price.request as OptionCalculation,
  );
  const result = useOptionCalculator(request);
  return (
    <OptionCalculator
      defaultRequest={request}
      onSubmit={setRequest}
      result={(result.data as OptionCalculationResult | undefined) ?? null}
      loading={result.isFetching}
      error={result.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
