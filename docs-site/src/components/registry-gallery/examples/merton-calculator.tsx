"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  MertonCalculator,
  type MertonCalculatorRequest,
} from "@/components/finstack/models/components/merton-calculator/merton-calculator";
import { useMertonReport } from "@/hooks/models/use-merton-report/use-merton-report";
import { useMertonModelValidator } from "@/hooks/models/use-merton-model-validator/use-merton-model-validator";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a caller validate stub.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Calculator density={density} />
    </WorkerPreview>
  );
}
// Host display choice: six decimal places of the returned decimals.
const sixPlaces = (value: number) => value.toFixed(6);
function Calculator({ density }: ExampleProps) {
  const validate = useMertonModelValidator();
  // Starts from the facade example firm; Calculate replaces the request.
  const [request, setRequest] = useState(
    data.merton.model.request as MertonCalculatorRequest,
  );
  const report = useMertonReport(request);
  return (
    <MertonCalculator
      defaultRequest={request}
      validate={validate}
      onSubmit={setRequest}
      report={report.data ?? null}
      loading={report.isFetching}
      error={report.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
