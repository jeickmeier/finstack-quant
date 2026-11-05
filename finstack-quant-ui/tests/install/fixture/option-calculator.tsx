"use client";
import { useState } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { OptionCalculator } from "@/components/finstack/models/components/option-calculator/option-calculator";

import data from "./data.json";
// The result is the supplied native fixture: the item installs without a worker.
const entry = data.options.black_scholes_price;
const result = (entry.envelope as { value: unknown })
  .value as modelsWire.OptionCalculationResult;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>Supplied valid fixture; standalone option calculator.</p>
      <OptionCalculator
        defaultRequest={entry.request as modelsWire.OptionCalculation}
        onSubmit={() => setCount((n) => n + 1)}
        result={result}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
