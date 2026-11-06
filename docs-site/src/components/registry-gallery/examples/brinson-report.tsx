"use client";
import type { ExampleProps } from "./props";
import {
  BrinsonReport,
  type BrinsonReportInputs,
} from "@/components/finstack/portfolio/components/brinson-report/brinson-report";
import { useBrinsonLinked } from "@/hooks/portfolio/use-brinson-linked/use-brinson-linked";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
// Host data: three quarters of Tech, Energy and Cash weights and returns for
// the portfolio and its benchmark (the portfolio host-parity golden).
const inputs = data.portfolioBrinson.inputs as BrinsonReportInputs;
function Live({ density }: ExampleProps) {
  const linked = useBrinsonLinked(inputs.periods);
  return (
    <BrinsonReport
      report={linked.data ?? null}
      periodLabels={inputs.periodLabels}
      loading={linked.isFetching}
      error={linked.error?.message}
      density={density}
    />
  );
}
// The hook sits outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with a supplied result.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Live density={density} />
    </WorkerPreview>
  );
}
