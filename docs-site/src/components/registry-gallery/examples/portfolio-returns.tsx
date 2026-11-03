"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  PortfolioReturns,
  type PortfolioReturnsInputs,
} from "@/components/finstack/portfolio/components/portfolio-returns/portfolio-returns";
import {
  useMwrXirr,
  useTwrrLinked,
  useTwrrModifiedDietz,
} from "@/hooks/portfolio/use-portfolio-returns/use-portfolio-returns";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// Host data: three annual returns over three years, the GIPS Handbook
// Modified-Dietz period and two contributions with a terminal value.
const inputs = data.portfolioReturns.inputs as PortfolioReturnsInputs;
function Live({ density }: ExampleProps) {
  const [linkedRequest, setLinkedRequest] = useState(inputs.linked);
  const [period, setPeriod] = useState(inputs.dietz);
  const [cashflows, setCashflows] = useState(inputs.xirr);
  const linked = useTwrrLinked(linkedRequest);
  const dietz = useTwrrModifiedDietz(period);
  const xirr = useMwrXirr(cashflows);
  return (
    <PortfolioReturns
      defaultInputs={inputs}
      onSubmitLinked={setLinkedRequest}
      onSubmitDietz={setPeriod}
      onSubmitXirr={setCashflows}
      linked={{
        value: linked.data ?? null,
        loading: linked.isFetching,
        error: linked.error?.message,
      }}
      dietz={{
        value: dietz.data ?? null,
        loading: dietz.isFetching,
        error: dietz.error?.message,
      }}
      xirr={{
        value: xirr.data ?? null,
        loading: xirr.isFetching,
        error: xirr.error?.message,
      }}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied results.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
