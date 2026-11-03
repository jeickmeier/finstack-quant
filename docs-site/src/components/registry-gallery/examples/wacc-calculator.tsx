"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  WaccCalculator,
  type WaccInputs,
} from "@/components/finstack/statements/components/wacc-calculator/wacc-calculator";
import { useWacc } from "@/hooks/statements/use-wacc/use-wacc";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const start = data.waccCalculator.cases.blended.inputs as WaccInputs;
function Live({ density }: ExampleProps) {
  const [inputs, setInputs] = useState<WaccInputs>(start);
  const [applied, setApplied] = useState<number | null>(null);
  const rate = useWacc(inputs);
  return (
    <div className="space-y-3">
      <WaccCalculator
        defaultInputs={start}
        onSubmit={setInputs}
        rate={rate.data ?? null}
        loading={rate.isFetching}
        error={rate.error?.message}
        onApply={setApplied}
        density={density}
      />
      {applied !== null && (
        <p role="status" className="text-sm text-muted-foreground">
          Passed on: {String(applied)}
        </p>
      )}
    </div>
  );
}
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied rate.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
