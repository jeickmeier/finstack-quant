"use client";
import { useState, type ComponentProps } from "react";
import { sixPlaces, type ExampleProps } from "./props";
import {
  CreditScorecard,
  type CreditScorecardRequest,
} from "@/components/finstack/models/components/credit-scorecard/credit-scorecard";
import { useCreditScore } from "@/hooks/models/use-credit-score/use-credit-score";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied score.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Scorecard density={density} />
    </WorkerPreview>
  );
}
const { description: _description, ...cases } = data.scorecard;
// The Rust unit-test inputs of every model; Score replaces the request.
const defaultInputs = Object.fromEntries(
  Object.entries(cases).map(([model, entry]) => [model, entry.request.input]),
) as unknown as ComponentProps<typeof CreditScorecard>["defaultInputs"];
function Scorecard({ density }: ExampleProps) {
  // Starts from the Zmijewski case, whose implied PD Rust grades.
  const [request, setRequest] = useState(
    cases.zmijewski_score.request as CreditScorecardRequest,
  );
  const score = useCreditScore(request);
  return (
    <CreditScorecard
      defaultModel={request.model}
      defaultInputs={defaultInputs}
      onSubmit={setRequest}
      score={score.data ?? null}
      loading={score.isFetching}
      error={score.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
